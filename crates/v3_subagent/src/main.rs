//! v3_subagent - Mini Claude Code: Subagent Mechanism (~900 lines)
//!
//! Core Philosophy: "Divide and Conquer with Context Isolation"
//! =============================================================
//! v2 adds planning. But for large tasks like "explore the codebase then
//! refactor auth", a single agent hits problems:
//!
//! The Problem - Context Pollution:
//! -------------------------------
//!     Single-Agent History:
//!       [exploring...] cat file1.rs -> 500 lines
//!       [exploring...] cat file2.rs -> 300 lines
//!       ... 15 more files ...
//!       [now refactoring...] "Wait, what did file1 contain?"
//!
//! The model's context fills with exploration details, leaving little room
//! for the actual task. This is "context pollution".
//!
//! The Solution - Subagents with Isolated Context:
//! ----------------------------------------------
//!     Main Agent History:
//!       [Task: explore codebase]
//!         -> Subagent explores 20 files (in its own context)
//!         -> Returns ONLY: "Auth in src/auth/, DB in src/models/"
//!       [now refactoring with clean context]
//!
//! Each subagent has:
//!   1. Its own fresh message history
//!   2. Filtered tools (explore can't write)
//!   3. Specialized system prompt
//!   4. Returns only final summary to parent
//!
//! The Key Insight:
//! ---------------
//!     Process isolation = Context isolation
//!
//! By spawning subtasks, we get:
//!   - Clean context for the main agent
//!   - Parallel exploration possible
//!   - Natural task decomposition
//!   - Same agent loop, different contexts
//!
//! Agent Type Registry:
//! -------------------
//!     | Type    | Tools               | Purpose                     |
//!     |---------|---------------------|---------------------------- |
//!     | explore | bash, read_file     | Read-only exploration       |
//!     | code    | all tools           | Full implementation access  |
//!     | plan    | bash, read_file     | Design without modifying    |
//!
//! Usage:
//!     cargo run -p v3_subagent

use anthropic::types::{
    ContentBlock, Message, MessagesRequestBuilder, Role, StopReason, SystemPrompt, Tool,
};
use anthropic::Client;
use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(feature = "readline"))]
use std::io::BufRead;

#[cfg(feature = "readline")]
use rustyline::error::ReadlineError;
#[cfg(feature = "readline")]
use rustyline::history::DefaultHistory;
#[cfg(feature = "readline")]
use rustyline::Editor;

// =============================================================================
// Thinking Animation (from v2, unchanged)
// =============================================================================

/// Spawn a thinking animation in a background thread
/// Returns a handle that stops the animation when dropped
fn spawn_thinking_animation() -> ThinkingAnimation {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let handle = thread::spawn(move || {
        let frames = vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 0;

        // Hide cursor
        print!("\x1B[?25l");
        io::stdout().flush().ok();

        while running_clone.load(Ordering::Relaxed) {
            let frame = frames[idx % frames.len()];
            print!("\r{} {}...", frame.bright_cyan(), "Thinking".bright_black());
            io::stdout().flush().ok();

            thread::sleep(Duration::from_millis(80));
            idx += 1;
        }

        // Clear the line and show cursor
        print!("\r\x1B[K\x1B[?25h");
        io::stdout().flush().ok();
    });

    ThinkingAnimation {
        running,
        handle: Some(handle),
    }
}

struct ThinkingAnimation {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Drop for ThinkingAnimation {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }
    }
}

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

Loop: plan -> act with tools -> report.

You can spawn subagents for complex subtasks:
{}

Rules:
- Use Task tool for subtasks that need focused exploration or implementation
- Use TodoWrite to track multi-step work
- Prefer tools over prose. Act, don't just explain.
- After finishing, summarize what changed."#,
            self.workdir.display(),
            get_agent_descriptions()
        )
    }
}

// =============================================================================
// Agent Type Registry - The core of subagent mechanism
// =============================================================================

#[derive(Debug, Clone)]
struct AgentConfig {
    description: String,
    tools: Vec<String>, // Tool whitelist, or vec!["*"] for all
    prompt: String,
}

/// Agent type registry
fn get_agent_types() -> HashMap<String, AgentConfig> {
    let mut types = HashMap::new();

    types.insert(
        "explore".to_string(),
        AgentConfig {
            description: "Read-only agent for exploring code, finding files, searching"
                .to_string(),
            tools: vec!["bash".to_string(), "read_file".to_string()],
            prompt: "You are an exploration agent. Search and analyze, but never modify files. Return a concise summary.".to_string(),
        },
    );

    types.insert(
        "code".to_string(),
        AgentConfig {
            description: "Full agent for implementing features and fixing bugs".to_string(),
            tools: vec!["*".to_string()], // All tools
            prompt: "You are a coding agent. Implement the requested changes efficiently."
                .to_string(),
        },
    );

    types.insert(
        "plan".to_string(),
        AgentConfig {
            description: "Planning agent for designing implementation strategies".to_string(),
            tools: vec!["bash".to_string(), "read_file".to_string()],
            prompt: "You are a planning agent. Analyze the codebase and output a numbered implementation plan. Do NOT make changes.".to_string(),
        },
    );

    types
}

/// Generate agent type descriptions for the Task tool
fn get_agent_descriptions() -> String {
    let types = get_agent_types();
    types
        .iter()
        .map(|(name, cfg)| format!("- {}: {}", name, cfg.description))
        .collect::<Vec<_>>()
        .join("\n")
}

// =============================================================================
// TodoManager (from v2, unchanged)
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
// Tool Definitions (v2 tools + Task)
// =============================================================================

fn create_base_tools() -> Vec<Tool> {
    vec![
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

/// Create the Task tool - NEW in v3
fn create_task_tool() -> Tool {
    let agent_types = get_agent_types();
    let type_names: Vec<String> = agent_types.keys().cloned().collect();

    Tool {
        name: "Task".to_string(),
        description: format!(
            r#"Spawn a subagent for a focused subtask.

Subagents run in ISOLATED context - they don't see parent's history.
Use this to keep the main conversation clean.

Agent types:
{}

Example uses:
- Task(explore): "Find all files using the auth module"
- Task(plan): "Design a migration strategy for the database"
- Task(code): "Implement the user registration form"
"#,
            get_agent_descriptions()
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "Short task name (3-5 words) for progress display"
                },
                "prompt": {
                    "type": "string",
                    "description": "Detailed instructions for the subagent"
                },
                "agent_type": {
                    "type": "string",
                    "enum": type_names,
                    "description": "Type of agent to spawn"
                }
            },
            "required": ["description", "prompt", "agent_type"]
        }),
    }
}

/// Get all tools for main agent (includes Task)
fn create_all_tools() -> Vec<Tool> {
    let mut tools = create_base_tools();
    tools.push(create_task_tool());
    tools
}

/// Filter tools based on agent type
fn get_tools_for_agent(agent_type: &str) -> Vec<Tool> {
    let agent_types = get_agent_types();
    let config = match agent_types.get(agent_type) {
        Some(cfg) => cfg,
        None => return Vec::new(),
    };

    let base_tools = create_base_tools();

    // If "*", return all base tools (but NOT Task to prevent recursion in this demo)
    if config.tools.contains(&"*".to_string()) {
        return base_tools;
    }

    // Filter to allowed tools
    base_tools
        .into_iter()
        .filter(|t| config.tools.contains(&t.name))
        .collect()
}

// =============================================================================
// Tool Implementations (from v2, unchanged)
// =============================================================================

/// Safely truncate a string at a UTF-8 character boundary.
fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

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

// =============================================================================
// Subagent Execution - The heart of v3
// =============================================================================

/// Execute a subagent task with isolated context.
///
/// This is the core of the subagent mechanism:
///
/// 1. Create isolated message history (KEY: no parent context!)
/// 2. Use agent-specific system prompt
/// 3. Filter available tools based on agent type
/// 4. Run the same query loop as main agent
/// 5. Return ONLY the final text (not intermediate details)
async fn run_task(
    client: &Client,
    config: &Config,
    todo_manager: &TodoManager,
    description: &str,
    prompt: &str,
    agent_type: &str,
) -> String {
    let agent_types = get_agent_types();
    let agent_config = match agent_types.get(agent_type) {
        Some(cfg) => cfg,
        None => return format!("Error: Unknown agent type '{}'", agent_type),
    };

    // Agent-specific system prompt
    let sub_system = format!(
        r#"You are a {} subagent at {}.

{}

Complete the task and return a clear, concise summary."#,
        agent_type,
        config.workdir.display(),
        agent_config.prompt
    );

    // Filtered tools for this agent type
    let sub_tools = get_tools_for_agent(agent_type);

    // ISOLATED message history - this is the key!
    let mut sub_messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text(prompt)],
    }];

    // Progress tracking
    println!("  {} {}", format!("[{}]", agent_type).bright_magenta(), description);
    let start = Instant::now();
    let mut tool_count = 0;

    // Run the same agent loop (silently - don't print details to main chat)
    loop {
        let request = MessagesRequestBuilder::new(&config.model, sub_messages.clone(), 8000)
            .system(SystemPrompt::Text(sub_system.clone()))
            .tools(sub_tools.clone())
            .build();

        let request = match request {
            Ok(r) => r,
            Err(e) => return format!("Error building request: {}", e),
        };

        let response = match client.messages(request).await {
            Ok(r) => r,
            Err(e) => return format!("Error calling API: {}", e),
        };

        if response.stop_reason != Some(StopReason::ToolUse) {
            // Extract final text
            for block in &response.content {
                if let ContentBlock::Text { text } = block {
                    let elapsed = start.elapsed();
                    println!(
                        "  {} {} - done ({} tools, {:.1}s)",
                        format!("[{}]", agent_type).bright_magenta(),
                        description,
                        tool_count,
                        elapsed.as_secs_f64()
                    );
                    return text.clone();
                }
            }
            return "(subagent returned no text)".to_string();
        }

        // Execute tool calls
        let mut results = Vec::new();
        for block in &response.content {
            if let ContentBlock::ToolUse { id, name, input } = block {
                tool_count += 1;
                let output = execute_tool(config, todo_manager, name, input);

                results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    is_error: None,
                    content: anthropic::types::ToolResultContent::Text(output),
                });

                // Update progress display (in-place)
                let elapsed = start.elapsed();
                print!(
                    "\r  {} {} ... {} tools, {:.1}s",
                    format!("[{}]", agent_type).bright_magenta(),
                    description,
                    tool_count,
                    elapsed.as_secs_f64()
                );
                io::stdout().flush().ok();
            }
        }

        print!("\r"); // Clear the progress line
        io::stdout().flush().ok();

        sub_messages.push(Message {
            role: Role::Assistant,
            content: response.content,
        });
        sub_messages.push(Message {
            role: Role::User,
            content: results,
        });
    }
}

fn execute_tool(
    config: &Config,
    todo_manager: &TodoManager,
    name: &str,
    input: &serde_json::Value,
) -> String {
    match name {
        "bash" => {
            if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
                run_bash(&config.workdir, command)
            } else {
                "Error: Missing 'command' parameter".to_string()
            }
        }
        "read_file" => {
            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                let limit = input.get("limit").and_then(|v| v.as_i64());
                run_read(&config.workdir, path, limit)
            } else {
                "Error: Missing 'path' parameter".to_string()
            }
        }
        "write_file" => {
            if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
                    run_write(&config.workdir, path, content)
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
                        run_edit(&config.workdir, path, old_text, new_text)
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

// For Task tool, we need async execution
async fn execute_tool_async(
    client: &Client,
    config: &Config,
    todo_manager: &TodoManager,
    name: &str,
    input: &serde_json::Value,
) -> String {
    if name == "Task" {
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("subtask");
        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let agent_type = input
            .get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("explore");

        run_task(client, config, todo_manager, description, prompt, agent_type).await
    } else {
        execute_tool(config, todo_manager, name, input)
    }
}

// =============================================================================
// Main Agent Loop (with subagent support)
// =============================================================================

async fn agent_loop(
    client: &Client,
    config: &Config,
    todo_manager: &TodoManager,
    messages: &mut Vec<Message>,
) -> Result<()> {
    let tools = create_all_tools();

    loop {
        let request = MessagesRequestBuilder::new(&config.model, messages.clone(), 8000)
            .system(SystemPrompt::Text(config.system_prompt()))
            .tools(tools.clone())
            .build()?;

        let start = Instant::now();
        let _animation = spawn_thinking_animation();

        let api_call = client.messages(request);
        let timeout_duration = Duration::from_secs(600);

        let response = match tokio::time::timeout(timeout_duration, api_call).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                drop(_animation);
                eprintln!("\n{}: {}", "API Error".bright_red(), e);
                return Err(e.into());
            }
            Err(_) => {
                drop(_animation);
                eprintln!(
                    "\n{}: {}",
                    "API Error".bright_red(),
                    "Request timed out after 10 minutes"
                );
                return Err(anyhow::anyhow!("Request timed out after 10 minutes"));
            }
        };

        let elapsed = start.elapsed();
        drop(_animation);

        let usage = &response.usage;
        println!(
            "{}",
            format!(
                "in: {} out: {} {:.1}s",
                usage.input_tokens,
                usage.output_tokens,
                elapsed.as_secs_f64()
            )
            .bright_black()
        );

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

        for (id, name, input) in tool_calls {
            // Display tool call
            let tool_display = match name.as_str() {
                "Task" => format!("{} {}", ">".bright_blue(), name.bright_magenta()),
                "TodoWrite" => format!("{} {}", ">".bright_blue(), name.bright_magenta()),
                "bash" => format!("{} {}", ">".bright_blue(), name.bright_yellow()),
                _ => format!("{} {}", ">".bright_blue(), name.bright_cyan()),
            };
            println!("\n{}", tool_display);

            let output = execute_tool_async(client, config, todo_manager, &name, &input).await;

            // Display output
            let preview = if name == "TodoWrite" || name == "Task" {
                output.clone()
            } else if output.len() > 300 {
                format!("{}...", safe_truncate(&output, 300))
            } else {
                output.clone()
            };

            if output.starts_with("Error:") {
                println!("{}", preview.bright_red());
            } else if name == "TodoWrite" {
                println!("{}", preview.bright_green());
            } else if name == "Task" {
                // Task output already printed by run_task
            } else {
                println!("  {}", preview.bright_black());
            }

            results.push(ContentBlock::ToolResult {
                tool_use_id: id,
                is_error: None,
                content: anthropic::types::ToolResultContent::Text(output),
            });
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
// Client initialization (from v2, unchanged)
// =============================================================================

fn create_client() -> Result<Client> {
    dotenvy::dotenv().ok();

    let api_key = env::var("ANTHROPIC_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))
        .map_err(|_| {
            anyhow::anyhow!("Missing API key: set ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN")
        })?;

    let mut builder = anthropic::client::ClientBuilder::new().api_key(api_key);

    if let Ok(base_url) = env::var("ANTHROPIC_API_BASE").or_else(|_| env::var("ANTHROPIC_BASE_URL"))
    {
        builder = builder.api_base(base_url);
    }

    if let Ok(api_version) = env::var("ANTHROPIC_API_VERSION") {
        builder = builder.api_version(api_version);
    }

    builder = builder.timeout(Duration::from_secs(600));

    let client = builder.build()?;
    Ok(client)
}

// =============================================================================
// Input handling (from v2, unchanged)
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
            "Mini Claude Code v3 (with Subagents) - {}",
            config.workdir.display()
        )
        .bright_green()
    );
    println!(
        "{}",
        format!("Agent types: {}", get_agent_types().keys().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))
            .bright_black()
    );
    println!("{}\n", "Type 'exit' to quit.".bright_black());

    let mut history: Vec<Message> = Vec::new();

    #[cfg(feature = "readline")]
    let mut rl = Editor::<(), DefaultHistory>::new()?;

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

        history.push(Message {
            role: Role::User,
            content: vec![ContentBlock::text(user_input)],
        });

        if let Err(e) = agent_loop(&client, &config, &todo_manager, &mut history).await {
            eprintln!("{}: {}", "Error".bright_red(), e);
        }

        println!();
    }

    Ok(())
}
