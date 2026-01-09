//! v2_todo_agent - Mini Claude Code: Structured Planning (~500 lines)
//!
//! Core Philosophy: "Make Plans Visible"
//! =====================================
//! v1 works great for simple tasks. But ask it to "refactor auth, add tests,
//! update docs" and watch what happens. Without explicit planning, the model:
//!   - Jumps between tasks randomly
//!   - Forgets completed steps
//!   - Loses focus mid-way
//!
//! The Problem - "Context Fade":
//! ----------------------------
//! In v1, plans exist only in the model's "head":
//!
//!     v1: "I'll do A, then B, then C"  (invisible)
//!         After 10 tool calls: "Wait, what was I doing?"
//!
//! The Solution - TodoWrite Tool:
//! -----------------------------
//! v2 adds ONE new tool that fundamentally changes how the agent works:
//!
//!     v2:
//!       [ ] Refactor auth module
//!       [>] Add unit tests         <- Currently working on this
//!       [ ] Update documentation
//!
//! Now both YOU and the MODEL can see the plan. The model can:
//!   - Update status as it works
//!   - See what's done and what's next
//!   - Stay focused on one task at a time
//!
//! Key Constraints (not arbitrary - these are guardrails):
//! ------------------------------------------------------
//!     | Rule              | Why                              |
//!     |-------------------|----------------------------------|
//!     | Max 20 items      | Prevents infinite task lists     |
//!     | One in_progress   | Forces focus on one thing        |
//!     | Required fields   | Ensures structured output        |
//!
//! The Deep Insight:
//! ----------------
//! > "Structure constrains AND enables."
//!
//! Todo constraints (max items, one in_progress) ENABLE (visible plan, tracked progress).
//!
//! Usage:
//!     cargo run -p v2_todo_agent

use anthropic::types::{
    ContentBlock, Message, MessagesRequestBuilder, Role, StopReason, SystemPrompt, Tool,
};
use anthropic::Client;
use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

#[cfg(not(feature = "readline"))]
use std::io::BufRead;

#[cfg(feature = "readline")]
use rustyline::error::ReadlineError;
#[cfg(feature = "readline")]
use rustyline::history::DefaultHistory;
#[cfg(feature = "readline")]
use rustyline::Editor;

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

WORKFLOW (Required for multi-step tasks):
1. PLAN: Use TodoWrite to create a visible task list
2. ACT: Execute tools to complete current task
3. UPDATE: Mark tasks complete, move to next
4. REPORT: Summarize what changed when done

TODO RULES (Enforced):
- Use TodoWrite for ANY task with 2+ steps
- Maximum 20 todo items (forces focused planning)
- Only ONE task can be "in_progress" at a time (enforces focus)
- Required fields for each item:
  * content: "What to do" (clear description)
  * status: "pending" | "in_progress" | "completed"
  * activeForm: "Doing it now" (present tense, shown during work)

TODO BEST PRACTICES:
- Mark task "in_progress" BEFORE starting work
- Mark task "completed" IMMEDIATELY after finishing
- Update activeForm to show current action
- Break large tasks into smaller steps
- Remove or mark completed tasks that are no longer relevant

TOOL USAGE:
- Prefer tools over explanations (ACT, don't just describe)
- Use bash for exploration: ls, find, grep, git
- Use read_file to understand code before changing
- Use edit_file for surgical changes, write_file for new files
- Never invent file paths - verify with bash first

OUTPUT:
- After completing all tasks, provide a brief summary
- Focus on what changed, not what you did
"#,
            self.workdir.display()
        )
    }
}

// =============================================================================
// TodoManager - The core addition in v2
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TodoItem {
    content: String,
    status: TodoStatus,
    #[serde(rename = "activeForm")]
    active_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// Manages a structured task list with enforced constraints.
///
/// Key Design Decisions:
/// --------------------
/// 1. Max 20 items: Prevents the model from creating endless lists
/// 2. One in_progress: Forces focus - can only work on ONE thing at a time
/// 3. Required fields: Each item needs content, status, and activeForm
///
/// The activeForm field deserves explanation:
/// - It's the PRESENT TENSE form of what's happening
/// - Shown when status is "in_progress"
/// - Example: content="Add tests", activeForm="Adding unit tests..."
///
/// This gives real-time visibility into what the agent is doing.
struct TodoManager {
    items: Arc<Mutex<Vec<TodoItem>>>,
}

impl TodoManager {
    fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Validate and update the todo list.
    ///
    /// The model sends a complete new list each time. We validate it,
    /// store it, and return a rendered view that the model will see.
    ///
    /// Validation Rules:
    /// - Each item must have: content, status, activeForm
    /// - Status must be: pending | in_progress | completed
    /// - Only ONE item can be in_progress at a time
    /// - Maximum 20 items allowed
    fn update(&self, new_items: Vec<TodoItem>) -> Result<String> {
        let mut in_progress_count = 0;

        // Validate items
        for (i, item) in new_items.iter().enumerate() {
            if item.content.trim().is_empty() {
                anyhow::bail!("Item {}: content required", i);
            }
            if item.active_form.trim().is_empty() {
                anyhow::bail!("Item {}: activeForm required", i);
            }
            if item.status == TodoStatus::InProgress {
                in_progress_count += 1;
            }
        }

        // Enforce constraints
        if new_items.len() > 20 {
            anyhow::bail!("Max 20 todos allowed");
        }
        if in_progress_count > 1 {
            anyhow::bail!("Only one task can be in_progress at a time");
        }

        // Update stored items
        *self.items.lock().unwrap() = new_items;

        Ok(self.render())
    }

    /// Render the todo list as human-readable text.
    ///
    /// Format:
    ///     [x] Completed task
    ///     [>] In progress task <- Doing something...
    ///     [ ] Pending task
    ///
    ///     (2/3 completed)
    fn render(&self) -> String {
        let items = self.items.lock().unwrap();

        if items.is_empty() {
            return "No todos.".to_string();
        }

        let mut lines = Vec::new();
        for item in items.iter() {
            let line = match item.status {
                TodoStatus::Completed => format!("[x] {}", item.content),
                TodoStatus::InProgress => format!("[>] {} <- {}", item.content, item.active_form),
                TodoStatus::Pending => format!("[ ] {}", item.content),
            };
            lines.push(line);
        }

        let completed = items
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count();
        lines.push(format!("\n({}/{} completed)", completed, items.len()));

        lines.join("\n")
    }
}

// =============================================================================
// Tool Definitions (v1 tools + TodoWrite)
// =============================================================================

fn create_tools() -> Vec<Tool> {
    vec![
        // v1 tools (unchanged)
        Tool {
            name: "bash".to_string(),
            description: "Run a shell command.".to_string(),
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
        Tool {
            name: "read_file".to_string(),
            description: "Read file contents.".to_string(),
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
        Tool {
            name: "write_file".to_string(),
            description: "Write content to file.".to_string(),
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
        Tool {
            name: "edit_file".to_string(),
            description: "Replace exact text in file.".to_string(),
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
        // NEW in v2: TodoWrite
        Tool {
            name: "TodoWrite".to_string(),
            description: "Update the task list. Use to plan and track progress.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "description": "Complete list of tasks (replaces existing)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "Task description"
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"],
                                    "description": "Task status"
                                },
                                "activeForm": {
                                    "type": "string",
                                    "description": "Present tense action, e.g. 'Reading files'"
                                }
                            },
                            "required": ["content", "status", "activeForm"]
                        }
                    }
                },
                "required": ["items"]
            }),
        },
    ]
}

// =============================================================================
// Tool Implementations
// =============================================================================

/// Safely truncate a string at a UTF-8 character boundary.
///
/// Unlike `&s[..n]` which panics if n is not at a character boundary,
/// this function finds the largest valid boundary <= max_bytes.
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    // Find the last character boundary at or before max_bytes
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }

    &s[..boundary]
}

fn safe_path(workdir: &Path, relative_path: &str) -> Result<PathBuf> {
    let path = workdir.join(relative_path);
    let canonical = path.canonicalize().or_else(|_| {
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

fn run_bash(workdir: &Path, command: &str) -> String {
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
            } else if combined.len() > 50000 {
                format!("{}...", safe_truncate(&combined, 50000))
            } else {
                combined
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

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

                if output.len() > 50000 {
                    format!("{}...", safe_truncate(&output, 50000))
                } else {
                    output
                }
            }
            Err(e) => format!("Error: {}", e),
        },
        Err(e) => format!("Error: {}", e),
    }
}

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

fn run_edit(workdir: &Path, path: &str, old_text: &str, new_text: &str) -> String {
    match safe_path(workdir, path) {
        Ok(safe_path) => match fs::read_to_string(&safe_path) {
            Ok(content) => {
                if !content.contains(old_text) {
                    return format!("Error: Text not found in {}", path);
                }

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

fn run_todo(todo_manager: &TodoManager, items: Vec<TodoItem>) -> String {
    match todo_manager.update(items) {
        Ok(rendered) => rendered,
        Err(e) => format!("Error: {}", e),
    }
}

fn execute_tool(
    workdir: &Path,
    todo_manager: &TodoManager,
    name: &str,
    input: &serde_json::Value,
) -> String {
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
        "TodoWrite" => {
            if let Some(items_value) = input.get("items") {
                match serde_json::from_value::<Vec<TodoItem>>(items_value.clone()) {
                    Ok(items) => run_todo(todo_manager, items),
                    Err(e) => format!("Error parsing todo items: {}", e),
                }
            } else {
                "Error: Missing 'items' parameter".to_string()
            }
        }
        _ => format!("Unknown tool: {}", name),
    }
}

// =============================================================================
// Agent Loop (with todo tracking)
// =============================================================================

async fn agent_loop(
    client: &Client,
    config: &Config,
    todo_manager: &TodoManager,
    messages: &mut Vec<Message>,
    rounds_without_todo: &mut usize,
) -> Result<()> {
    let tools = create_tools();

    loop {
        let request = MessagesRequestBuilder::new(&config.model, messages.clone(), 8000)
            .system(SystemPrompt::Text(config.system_prompt()))
            .tools(tools.clone())
            .build()?;

        // Show "thinking" indicator
        print!("{}", "Thinking...".bright_black());
        io::stdout().flush().ok();

        // Wrap API call with explicit timeout (10 minutes)
        let api_call = client.messages(request);
        let timeout_duration = std::time::Duration::from_secs(600);

        let response = match tokio::time::timeout(timeout_duration, api_call).await {
            Ok(Ok(resp)) => {
                // Clear "thinking" indicator
                print!("\r{}\r", " ".repeat(20));
                io::stdout().flush().ok();
                resp
            }
            Ok(Err(e)) => {
                // Clear "thinking" indicator
                print!("\r{}\r", " ".repeat(20));
                io::stdout().flush().ok();

                // Display user-friendly error message
                eprintln!("\n{}: {}", "API Error".bright_red(), e);

                // Check for common errors and provide helpful messages
                let error_msg = e.to_string();
                if error_msg.contains("余额不足") || error_msg.contains("insufficient") {
                    eprintln!(
                        "{}",
                        "Hint: Your API account balance is insufficient. Please recharge."
                            .bright_yellow()
                    );
                } else if error_msg.contains("unauthorized") || error_msg.contains("401") {
                    eprintln!(
                        "{}",
                        "Hint: API key may be invalid. Check your ANTHROPIC_API_KEY."
                            .bright_yellow()
                    );
                } else if error_msg.contains("timeout") {
                    eprintln!(
                        "{}",
                        "Hint: Request timed out. The API server may be slow or unreachable."
                            .bright_yellow()
                    );
                } else if error_msg.contains("connection") {
                    eprintln!(
                        "{}",
                        "Hint: Network connection error. Check your internet connection."
                            .bright_yellow()
                    );
                }

                return Err(e.into());
            }
            Err(_) => {
                // Timeout occurred
                print!("\r{}\r", " ".repeat(20));
                io::stdout().flush().ok();

                eprintln!(
                    "\n{}: {}",
                    "API Error".bright_red(),
                    "Request timed out after 10 minutes"
                );
                eprintln!(
                    "{}",
                    "Hint: Request timed out. The task may be too complex or the API server is slow."
                        .bright_yellow()
                );

                return Err(anyhow::anyhow!("Request timed out after 10 minutes"));
            }
        };

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

        if response.stop_reason != Some(StopReason::ToolUse) {
            messages.push(Message {
                role: Role::Assistant,
                content: response.content,
            });
            return Ok(());
        }

        let mut results = Vec::new();
        let mut used_todo = false;

        for (id, name, input) in tool_calls {
            // Display tool call with different colors for different tools
            let tool_display = match name.as_str() {
                "TodoWrite" => format!("{} {}", ">".bright_blue(), name.bright_magenta()),
                "bash" => format!("{} {}", ">".bright_blue(), name.bright_yellow()),
                _ => format!("{} {}", ">".bright_blue(), name.bright_cyan()),
            };
            println!("\n{}", tool_display);

            let output = execute_tool(&config.workdir, todo_manager, &name, &input);

            // For TodoWrite, show full output; for others, truncate
            let preview = if name == "TodoWrite" {
                output.clone()
            } else if output.len() > 300 {
                format!("{}...", safe_truncate(&output, 300))
            } else {
                output.clone()
            };

            // Color the output based on success/error
            if output.starts_with("Error:") {
                println!("{}", preview.bright_red());
            } else if name == "TodoWrite" {
                println!("{}", preview.bright_green());
            } else {
                println!("  {}", preview.bright_black());
            }

            results.push(ContentBlock::ToolResult {
                tool_use_id: id,
                is_error: None,
                content: anthropic::types::ToolResultContent::Text(output),
            });

            if name == "TodoWrite" {
                used_todo = true;
            }
        }

        // Update counter
        if used_todo {
            *rounds_without_todo = 0;
        } else {
            *rounds_without_todo += 1;
        }

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
// Client initialization
// =============================================================================

/// Initialize API client with credentials from environment
/// Supports both ANTHROPIC_API_KEY and ANTHROPIC_AUTH_TOKEN
/// Supports both ANTHROPIC_API_BASE and ANTHROPIC_BASE_URL
fn create_client() -> Result<Client> {
    dotenvy::dotenv().ok();

    // Try standard env vars first, then fallback to alternative names
    let api_key = env::var("ANTHROPIC_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))
        .map_err(|_| {
            anyhow::anyhow!("Missing API key: set ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN")
        })?;

    let mut builder = anthropic::client::ClientBuilder::new().api_key(api_key);

    // Try both ANTHROPIC_API_BASE and ANTHROPIC_BASE_URL
    if let Ok(base_url) = env::var("ANTHROPIC_API_BASE").or_else(|_| env::var("ANTHROPIC_BASE_URL"))
    {
        builder = builder.api_base(base_url);
    }

    if let Ok(api_version) = env::var("ANTHROPIC_API_VERSION") {
        builder = builder.api_version(api_version);
    }

    // Set timeout to 10 minutes to allow for complex code generation
    // while still preventing indefinite hanging
    builder = builder.timeout(std::time::Duration::from_secs(600));

    let client = builder.build()?;
    Ok(client)
}

// =============================================================================
// Input handling
// =============================================================================

#[cfg(not(feature = "readline"))]
fn prompt_user() -> Result<String> {
    print!("{} ", "You:".bright_cyan());
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("Failed to read from stdin")?;

    Ok(line.trim().to_string())
}

#[cfg(feature = "readline")]
fn prompt_user_with_rl(editor: &mut Editor<(), DefaultHistory>) -> Result<String> {
    let readline = editor.readline(&format!("{} ", "You:".bright_cyan()));

    match readline {
        Ok(line) => {
            let _ = editor.add_history_entry(&line);
            Ok(line.trim().to_string())
        }
        Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => Ok("exit".to_string()),
        Err(err) => Err(anyhow::anyhow!("Readline error: {}", err)),
    }
}

// =============================================================================
// Main REPL
// =============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    let client = create_client()?;
    let todo_manager = TodoManager::new();

    println!(
        "{}",
        format!(
            "Mini Claude Code v2 (with Todos) - {}",
            config.workdir.display()
        )
        .bright_green()
    );
    println!("{}\n", "Type 'exit' to quit.".bright_black());

    let mut history: Vec<Message> = Vec::new();
    let mut first_message = true;
    let mut rounds_without_todo = 0;

    #[cfg(feature = "readline")]
    let mut rl = Editor::<(), DefaultHistory>::new()?;

    // Reminder messages - More detailed and actionable
    let initial_reminder = r#"<system-reminder>
For multi-step tasks, use the TodoWrite tool to track progress:

Example TodoWrite structure:
{
  "items": [
    {
      "content": "Read and analyze codebase structure",
      "status": "in_progress",
      "activeForm": "Analyzing codebase structure"
    },
    {
      "content": "Identify key components and patterns",
      "status": "pending",
      "activeForm": "Identifying components"
    },
    {
      "content": "Write analysis report",
      "status": "pending",
      "activeForm": "Writing report"
    }
  ]
}

Benefits:
- Visible plan for both you and me
- Track what's done and what's next
- Stay focused on one task at a time (only one "in_progress")
- Maximum 20 tasks to keep plans manageable
</system-reminder>"#;

    let nag_reminder = r#"<system-reminder>
It's been 10+ tool calls without updating the todo list.

Please update the TodoWrite to:
1. Mark completed tasks as "completed"
2. Update current task to "in_progress" with activeForm
3. Add any new tasks discovered during work

This helps maintain visibility and focus.
</system-reminder>"#;

    loop {
        let user_input = {
            #[cfg(feature = "readline")]
            {
                prompt_user_with_rl(&mut rl)?
            }
            #[cfg(not(feature = "readline"))]
            {
                prompt_user()?
            }
        };

        if user_input.is_empty()
            || matches!(user_input.to_lowercase().as_str(), "exit" | "quit" | "q")
        {
            break;
        }

        // Build user message with optional reminders
        let mut content = Vec::new();

        if first_message {
            content.push(ContentBlock::text(initial_reminder));
            first_message = false;
        } else if rounds_without_todo > 10 {
            content.push(ContentBlock::text(nag_reminder));
        }

        content.push(ContentBlock::text(user_input));

        history.push(Message {
            role: Role::User,
            content,
        });

        if let Err(e) = agent_loop(
            &client,
            &config,
            &todo_manager,
            &mut history,
            &mut rounds_without_todo,
        )
        .await
        {
            eprintln!("{}: {}", "Error".bright_red(), e);
        }

        println!();
    }

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_truncate_short_string() {
        let s = "Hello, World!";
        assert_eq!(safe_truncate(s, 100), "Hello, World!");
    }

    #[test]
    fn test_safe_truncate_ascii() {
        let s = "Hello, World! This is a test.";
        let truncated = safe_truncate(s, 13);
        assert_eq!(truncated, "Hello, World!");
    }

    #[test]
    fn test_safe_truncate_utf8() {
        // Chinese characters are 3 bytes each
        let s = "你好世界abc"; // 你好世界 = 12 bytes, a = 1 byte
        let truncated = safe_truncate(s, 13);
        // Should truncate to "你好世界a" (13 bytes), including the 'a'
        assert_eq!(truncated, "你好世界a");
        assert!(truncated.len() <= 13);

        // Test mid-character truncation: byte 13 would be in middle of 'b'
        let s2 = "你好世界😀"; // 你好世界 = 12 bytes, 😀 = 4 bytes (total 16)
        let truncated2 = safe_truncate(s2, 13);
        // Should truncate to just "你好世界" (12 bytes), not split the emoji
        assert_eq!(truncated2, "你好世界");
        assert!(truncated2.len() <= 13);
    }

    #[test]
    fn test_safe_truncate_emoji() {
        // Emoji are 4 bytes each
        let s = "😀😁😂abc";
        let truncated = safe_truncate(s, 9);
        // Should truncate to "😀😁" (8 bytes), not split into the third emoji
        assert_eq!(truncated, "😀😁");
    }

    #[test]
    fn test_create_tools_count() {
        let tools = create_tools();
        assert_eq!(
            tools.len(),
            5,
            "Should have 5 tools (4 from v1 + TodoWrite)"
        );
    }

    #[test]
    fn test_create_tools_names() {
        let tools = create_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"bash"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"edit_file"));
        assert!(names.contains(&"TodoWrite"));
    }

    #[test]
    fn test_todo_manager_new() {
        let manager = TodoManager::new();
        let rendered = manager.render();
        assert_eq!(rendered, "No todos.");
    }

    #[test]
    fn test_todo_manager_update_single_item() {
        let manager = TodoManager::new();
        let items = vec![TodoItem {
            content: "Test task".to_string(),
            status: TodoStatus::Pending,
            active_form: "Testing".to_string(),
        }];
        let result = manager.update(items);
        assert!(result.is_ok());
        let rendered = manager.render();
        assert!(rendered.contains("[ ] Test task"));
        assert!(rendered.contains("(0/1 completed)"));
    }

    #[test]
    fn test_todo_manager_multiple_items() {
        let manager = TodoManager::new();
        let items = vec![
            TodoItem {
                content: "Task 1".to_string(),
                status: TodoStatus::Completed,
                active_form: "Doing task 1".to_string(),
            },
            TodoItem {
                content: "Task 2".to_string(),
                status: TodoStatus::InProgress,
                active_form: "Doing task 2".to_string(),
            },
            TodoItem {
                content: "Task 3".to_string(),
                status: TodoStatus::Pending,
                active_form: "Doing task 3".to_string(),
            },
        ];
        let result = manager.update(items);
        assert!(result.is_ok());
        let rendered = manager.render();
        assert!(rendered.contains("[x] Task 1"));
        assert!(rendered.contains("[>] Task 2 <- Doing task 2"));
        assert!(rendered.contains("[ ] Task 3"));
        assert!(rendered.contains("(1/3 completed)"));
    }

    #[test]
    fn test_todo_manager_max_items_enforcement() {
        let manager = TodoManager::new();
        let items: Vec<TodoItem> = (0..21)
            .map(|i| TodoItem {
                content: format!("Task {}", i),
                status: TodoStatus::Pending,
                active_form: format!("Doing task {}", i),
            })
            .collect();
        let result = manager.update(items);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Max 20 todos"));
    }

    #[test]
    fn test_todo_manager_one_in_progress_enforcement() {
        let manager = TodoManager::new();
        let items = vec![
            TodoItem {
                content: "Task 1".to_string(),
                status: TodoStatus::InProgress,
                active_form: "Doing task 1".to_string(),
            },
            TodoItem {
                content: "Task 2".to_string(),
                status: TodoStatus::InProgress,
                active_form: "Doing task 2".to_string(),
            },
        ];
        let result = manager.update(items);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Only one task can be in_progress"));
    }

    #[test]
    fn test_todo_manager_empty_content_rejected() {
        let manager = TodoManager::new();
        let items = vec![TodoItem {
            content: "   ".to_string(), // Empty/whitespace only
            status: TodoStatus::Pending,
            active_form: "Testing".to_string(),
        }];
        let result = manager.update(items);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("content required"));
    }

    #[test]
    fn test_config_system_prompt() {
        let config = Config {
            model: "test-model".to_string(),
            workdir: PathBuf::from("/test/path"),
        };
        let prompt = config.system_prompt();
        assert!(prompt.contains("/test/path"));
        assert!(prompt.contains("coding agent"));
        assert!(prompt.contains("TodoWrite"));
    }

    #[test]
    fn test_run_bash_simple() {
        let workdir = std::env::current_dir().unwrap();
        let output = run_bash(&workdir, "echo 'test'");
        assert!(output.contains("test"));
    }

    #[test]
    fn test_run_bash_dangerous_blocked() {
        let workdir = std::env::current_dir().unwrap();
        let output = run_bash(&workdir, "rm -rf /");
        assert!(output.contains("Dangerous command blocked"));
    }

    #[test]
    fn test_safe_path_valid() {
        let workdir = std::env::temp_dir();
        let result = safe_path(&workdir, "test.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_safe_path_escape_attempt() {
        let workdir = std::env::temp_dir();
        let result = safe_path(&workdir, "../../../etc/passwd");
        assert!(result.is_err());
    }
}
