//! v1_basic_agent - Mini Claude Code: Model as Agent (~300 lines)
//!
//! Core Philosophy: "The Model IS the Agent"
//! =========================================
//! The secret of Claude Code, Cursor Agent, Codex CLI? There is no secret.
//!
//! Strip away the CLI polish, progress bars, permission systems. What remains
//! is surprisingly simple: a LOOP that lets the model call tools until done.
//!
//! Traditional Assistant:
//!     User -> Model -> Text Response
//!
//! Agent System:
//!     User -> Model -> [Tool -> Result]* -> Response
//!                           ^________|
//!
//! The asterisk (*) matters! The model calls tools REPEATEDLY until it decides
//! the task is complete. This transforms a chatbot into an autonomous agent.
//!
//! KEY INSIGHT: The model is the decision-maker. Code just provides tools and
//! runs the loop. The model decides:
//!   - Which tools to call
//!   - In what order
//!   - When to stop
//!
//! The Four Essential Tools:
//! ------------------------
//! Claude Code has ~20 tools. But these 4 cover 90% of use cases:
//!
//!     | Tool       | Purpose              | Example                    |
//!     |------------|----------------------|----------------------------|
//!     | bash       | Run any command      | npm install, git status    |
//!     | read_file  | Read file contents   | View src/index.ts          |
//!     | write_file | Create/overwrite     | Create README.md           |
//!     | edit_file  | Surgical changes     | Replace a function         |
//!
//! With just these 4 tools, the model can:
//!   - Explore codebases (bash: find, grep, ls)
//!   - Understand code (read_file)
//!   - Make changes (write_file, edit_file)
//!   - Run anything (bash: python, npm, make)
//!
//! Usage:
//!     cargo run --bin v1_basic_agent

use anthropic::types::{
    ContentBlock, Message, MessagesRequestBuilder, Role, StopReason, SystemPrompt, Tool,
};
use anthropic::Client;
use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::json;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// =============================================================================
// Configuration
// =============================================================================

struct Config {
    model: String,
    workdir: PathBuf,
}

impl Config {
    fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let model =
            env::var("MODEL_NAME").unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
        let workdir = env::current_dir().context("Failed to get current directory")?;

        Ok(Self { model, workdir })
    }

    fn system_prompt(&self) -> String {
        format!(
            r#"You are a coding agent at {}.

Loop: think briefly -> use tools -> report results.

Rules:
- Prefer tools over prose. Act, don't just explain.
- Never invent file paths. Use bash ls/find first if unsure.
- Make minimal changes. Don't over-engineer.
- After finishing, summarize what changed."#,
            self.workdir.display()
        )
    }
}

// =============================================================================
// Tool Definitions - 4 tools cover 90% of coding tasks
// =============================================================================

fn create_tools() -> Vec<Tool> {
    vec![
        // Tool 1: Bash - The gateway to everything
        // Can run any command: git, npm, python, curl, etc.
        Tool {
            name: "bash".to_string(),
            description: "Run a shell command. Use for: ls, find, grep, git, npm, python, etc."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        // Tool 2: Read File - For understanding existing code
        // Returns file content with optional line limit for large files
        Tool {
            name: "read_file".to_string(),
            description: "Read file contents. Returns UTF-8 text.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max lines to read (default: all)"
                    }
                },
                "required": ["path"]
            }),
        },
        // Tool 3: Write File - For creating new files or complete rewrites
        // Creates parent directories automatically
        Tool {
            name: "write_file".to_string(),
            description: "Write content to a file. Creates parent directories if needed."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path for the file"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        // Tool 4: Edit File - For surgical changes to existing code
        // Uses exact string matching for precise edits
        Tool {
            name: "edit_file".to_string(),
            description: "Replace exact text in a file. Use for surgical edits.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Exact text to find (must match precisely)"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text"
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        },
    ]
}

// =============================================================================
// Tool Implementations
// =============================================================================

/// Ensure path stays within workspace (security measure).
///
/// Prevents the model from accessing files outside the project directory.
/// Resolves relative paths and checks they don't escape via '../'.
fn safe_path(workdir: &Path, relative_path: &str) -> Result<PathBuf> {
    let path = workdir.join(relative_path);
    let canonical = path.canonicalize().or_else(|_| {
        // If file doesn't exist yet, check parent directory
        if let Some(parent) = path.parent() {
            if parent.exists() {
                Ok(parent.canonicalize()?.join(
                    path.file_name()
                        .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
                ))
            } else {
                Err(anyhow::anyhow!("Parent directory does not exist"))
            }
        } else {
            Err(anyhow::anyhow!("Invalid path"))
        }
    })?;

    if !canonical.starts_with(workdir) {
        anyhow::bail!("Path escapes workspace: {}", relative_path);
    }

    Ok(canonical)
}

/// Execute shell command with safety checks.
///
/// Security: Blocks obviously dangerous commands.
/// Timeout: 60 seconds to prevent hanging.
/// Output: Truncated to 50KB to prevent context overflow.
fn run_bash(workdir: &Path, command: &str) -> String {
    // Basic safety - block dangerous patterns
    let dangerous = ["rm -rf /", "sudo", "shutdown", "reboot", "> /dev/"];
    if dangerous.iter().any(|d| command.contains(d)) {
        return "Error: Dangerous command blocked".to_string();
    }

    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(workdir)
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr).trim().to_string();

            if combined.is_empty() {
                "(no output)".to_string()
            } else {
                // Truncate to 50KB
                if combined.len() > 50000 {
                    format!("{}...", &combined[..50000])
                } else {
                    combined
                }
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

/// Read file contents with optional line limit.
///
/// For large files, use limit to read just the first N lines.
/// Output truncated to 50KB to prevent context overflow.
fn run_read(workdir: &Path, path: &str, limit: Option<i64>) -> String {
    match safe_path(workdir, path) {
        Ok(safe_path) => match fs::read_to_string(&safe_path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                let output = if let Some(limit) = limit {
                    if limit > 0 && (limit as usize) < total_lines {
                        let limited_lines: Vec<&str> =
                            lines.iter().take(limit as usize).copied().collect();
                        format!(
                            "{}\n... ({} more lines)",
                            limited_lines.join("\n"),
                            total_lines - limit as usize
                        )
                    } else {
                        content
                    }
                } else {
                    content
                };

                // Truncate to 50KB
                if output.len() > 50000 {
                    format!("{}...", &output[..50000])
                } else {
                    output
                }
            }
            Err(e) => format!("Error: {}", e),
        },
        Err(e) => format!("Error: {}", e),
    }
}

/// Write content to file, creating parent directories if needed.
///
/// This is for complete file creation/overwrite.
/// For partial edits, use edit_file instead.
fn run_write(workdir: &Path, path: &str, content: &str) -> String {
    match safe_path(workdir, path) {
        Ok(safe_path) => {
            if let Some(parent) = safe_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return format!("Error creating parent directories: {}", e);
                }
            }

            match fs::write(&safe_path, content) {
                Ok(_) => format!("Wrote {} bytes to {}", content.len(), path),
                Err(e) => format!("Error: {}", e),
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

/// Replace exact text in a file (surgical edit).
///
/// Uses exact string matching - the old_text must appear verbatim.
/// Only replaces the first occurrence to prevent accidental mass changes.
fn run_edit(workdir: &Path, path: &str, old_text: &str, new_text: &str) -> String {
    match safe_path(workdir, path) {
        Ok(safe_path) => match fs::read_to_string(&safe_path) {
            Ok(content) => {
                if !content.contains(old_text) {
                    return format!("Error: Text not found in {}", path);
                }

                // Replace only first occurrence for safety
                let new_content = content.replacen(old_text, new_text, 1);

                match fs::write(&safe_path, new_content) {
                    Ok(_) => format!("Edited {}", path),
                    Err(e) => format!("Error: {}", e),
                }
            }
            Err(e) => format!("Error: {}", e),
        },
        Err(e) => format!("Error: {}", e),
    }
}

/// Dispatch tool call to the appropriate implementation.
///
/// This is the bridge between the model's tool calls and actual execution.
/// Each tool returns a string result that goes back to the model.
fn execute_tool(workdir: &Path, name: &str, input: &serde_json::Value) -> String {
    match name {
        "bash" => {
            if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
                run_bash(workdir, command)
            } else {
                "Error: Missing 'command' parameter".to_string()
            }
        }
        "read_file" => {
            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                let limit = input.get("limit").and_then(|v| v.as_i64());
                run_read(workdir, path, limit)
            } else {
                "Error: Missing 'path' parameter".to_string()
            }
        }
        "write_file" => {
            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
                    run_write(workdir, path, content)
                } else {
                    "Error: Missing 'content' parameter".to_string()
                }
            } else {
                "Error: Missing 'path' parameter".to_string()
            }
        }
        "edit_file" => {
            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                if let Some(old_text) = input.get("old_text").and_then(|v| v.as_str()) {
                    if let Some(new_text) = input.get("new_text").and_then(|v| v.as_str()) {
                        run_edit(workdir, path, old_text, new_text)
                    } else {
                        "Error: Missing 'new_text' parameter".to_string()
                    }
                } else {
                    "Error: Missing 'old_text' parameter".to_string()
                }
            } else {
                "Error: Missing 'path' parameter".to_string()
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}

// =============================================================================
// The Agent Loop - This is the CORE of everything
// =============================================================================

/// The complete agent in one function.
///
/// This is the pattern that ALL coding agents share:
///
///     while True:
///         response = model(messages, tools)
///         if no tool calls: return
///         execute tools, append results, continue
///
/// The model controls the loop:
///   - Keeps calling tools until stop_reason != "tool_use"
///   - Results become context (fed back as "user" messages)
///   - Memory is automatic (messages list accumulates history)
///
/// Why this works:
///   1. Model decides which tools, in what order, when to stop
///   2. Tool results provide feedback for next decision
///   3. Conversation history maintains context across turns
async fn agent_loop(client: &Client, config: &Config, messages: &mut Vec<Message>) -> Result<()> {
    let tools = create_tools();

    loop {
        // Step 1: Call the model
        let request = MessagesRequestBuilder::new(&config.model, messages.clone(), 8000)
            .system(SystemPrompt::Text(config.system_prompt()))
            .tools(tools.clone())
            .build()?;

        let response = client.messages(request).await?;

        // Step 2: Collect any tool calls and print text output
        let mut tool_calls = Vec::new();
        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    if !text.trim().is_empty() {
                        println!("{}", text);
                    }
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push((id.clone(), name.clone(), input.clone()));
                }
                _ => {}
            }
        }

        // Step 3: If no tool calls, task is complete
        if response.stop_reason != Some(StopReason::ToolUse) {
            messages.push(Message {
                role: Role::Assistant,
                content: response.content,
            });
            return Ok(());
        }

        // Step 4: Execute each tool and collect results
        let mut results = Vec::new();
        for (id, name, input) in tool_calls {
            // Display what's being executed
            println!(
                "\n{} {}: {:?}",
                ">".bright_blue(),
                name.bright_yellow(),
                input
            );

            // Execute and show result preview
            let output = execute_tool(&config.workdir, &name, &input);
            let preview = if output.len() > 200 {
                format!("{}...", &output[..200])
            } else {
                output.clone()
            };
            println!("  {}", preview.bright_black());

            // Collect result for the model
            results.push(ContentBlock::ToolResult {
                tool_use_id: id,
                is_error: None,
                content: anthropic::types::ToolResultContent::Text(output),
            });
        }

        // Step 5: Append to conversation and continue
        // Note: We append assistant's response, then user's tool results
        // This maintains the alternating user/assistant pattern
        messages.push(Message {
            role: Role::Assistant,
            content: response.content,
        });
        messages.push(Message {
            role: Role::User,
            content: results,
        });
    }
}

// =============================================================================
// Main REPL
// =============================================================================

/// Simple Read-Eval-Print Loop for interactive use.
///
/// The history list maintains conversation context across turns,
/// allowing multi-turn conversations with memory.
#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    // Initialize client - from_env() handles both API_KEY and BASE_URL
    let client = Client::from_env()?;

    println!(
        "{}",
        format!("Mini Claude Code v1 - {}", config.workdir.display()).bright_green()
    );
    println!("{}\n", "Type 'exit' to quit.".bright_black());

    let mut history: Vec<Message> = Vec::new();

    loop {
        print!("{} ", "You:".bright_cyan());
        io::stdout().flush()?;

        let mut user_input = String::new();
        io::stdin().read_line(&mut user_input)?;
        let user_input = user_input.trim();

        if user_input.is_empty()
            || matches!(user_input.to_lowercase().as_str(), "exit" | "quit" | "q")
        {
            break;
        }

        // Add user message to history
        history.push(Message {
            role: Role::User,
            content: vec![ContentBlock::text(user_input)],
        });

        // Run the agent loop
        if let Err(e) = agent_loop(&client, &config, &mut history).await {
            eprintln!("{}: {}", "Error".bright_red(), e);
        }

        println!(); // Blank line between turns
    }

    Ok(())
}
