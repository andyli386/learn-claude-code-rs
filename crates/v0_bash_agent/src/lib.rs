//! Library module for v0_bash_agent
//!
//! This module contains the core functionality that can be tested
//! and reused by other parts of the application.

use anthropic::types::{
    ContentBlock, Message, MessagesRequestBuilder, Role, StopReason, SystemPrompt, Tool,
};
use anthropic::Client;
use anyhow::Result;
use colored::Colorize;
use serde_json::json;
use std::env;
use std::process::{Command, Stdio};

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

/// Get current working directory
pub fn get_cwd() -> String {
    env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| ".".to_string())
}

/// The ONE tool that does everything
/// Notice how the description teaches the model common patterns AND how to spawn subagents
pub fn get_bash_tool() -> Tool {
    Tool {
        name: "bash".to_string(),
        description: r#"Execute shell command. Common patterns:
- Read: cat/head/tail, grep/find/rg/ls, wc -l
- Write: echo 'content' > file, sed -i 's/old/new/g' file
- Subagent: v0_bash_agent 'task description' (spawns isolated agent, returns summary)"#
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"}
            },
            "required": ["command"]
        }),
    }
}

/// System prompt teaches the model HOW to use bash effectively
/// Notice the subagent guidance - this is how we get hierarchical task decomposition
pub fn get_system_prompt() -> String {
    format!(
        r#"You are a CLI agent at {}. Solve problems using bash commands.

Rules:
- Prefer tools over prose. Act first, explain briefly after.
- Read files: cat, grep, find, rg, ls, head, tail
- Write files: echo '...' > file, sed -i, or cat << 'EOF' > file
- Subagent: For complex subtasks, spawn a subagent to keep context clean:
  v0_bash_agent "explore src/ and summarize the architecture"

When to use subagent:
- Task requires reading many files (isolate the exploration)
- Task is independent and self-contained
- You want to avoid polluting current conversation with intermediate details

The subagent runs in isolation and returns only its final summary."#,
        get_cwd()
    )
}

/// Execute a bash command and return output
pub fn execute_bash(command: &str) -> String {
    let output = Command::new("bash")
        .arg("-c")
        .arg(command)
        .current_dir(get_cwd())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            format!("{}{}", stdout, stderr)
        }
        Err(e) => format!("Error executing command: {}", e),
    }
}

/// The complete agent loop
///
/// This is the core pattern that ALL coding agents share:
///     while not done:
///         response = model(messages, tools)
///         if no tool calls: return
///         execute tools, append results
///
/// Args:
///     client: Anthropic API client
///     prompt: User's request
///     history: Conversation history (mutable, shared across calls in interactive mode)
///
/// Returns:
///     Final text response from the model
pub async fn chat(
    client: &Client,
    model: &str,
    prompt: &str,
    history: &mut Vec<Message>,
) -> Result<String> {
    let tools = vec![get_bash_tool()];
    let system = get_system_prompt();

    // Add user message
    history.push(Message {
        role: Role::User,
        content: vec![ContentBlock::text(prompt)],
    });

    loop {
        // 1. Call the model with tools
        let request = MessagesRequestBuilder::new(model.to_string(), history.clone(), 8000)
            .system(SystemPrompt::Text(system.clone()))
            .tools(tools.clone())
            .build()?;

        // Wrap API call with explicit timeout (10 minutes)
        let api_call = client.messages(request);
        let timeout_duration = std::time::Duration::from_secs(600);

        let response = match tokio::time::timeout(timeout_duration, api_call).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
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
                eprintln!(
                    "\n{}: Request timed out after 10 minutes",
                    "API Error".bright_red()
                );
                eprintln!(
                    "{}",
                    "Hint: Request timed out. The task may be too complex or the API server is slow."
                        .bright_yellow()
                );

                return Err(anyhow::anyhow!("Request timed out after 10 minutes"));
            }
        };

        // 2. Build assistant message content (preserve both text and tool_use blocks)
        let mut assistant_content = vec![];
        for block in &response.content {
            assistant_content.push(block.clone());
        }

        history.push(Message {
            role: Role::Assistant,
            content: assistant_content,
        });

        // 3. If model didn't call tools, we're done
        if response.stop_reason != Some(StopReason::ToolUse) {
            let text = response
                .content
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::Text { text } = block {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");
            return Ok(text);
        }

        // 4. Execute each tool call and collect results
        let mut results = vec![];
        for block in &response.content {
            if let ContentBlock::ToolUse { id, name: _, input } = block {
                if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
                    // Execute command with timeout
                    let output = execute_bash(command);
                    let truncated_output = if output.len() > 50000 {
                        format!("{}... (truncated)", safe_truncate(&output, 50000))
                    } else {
                        output
                    };

                    results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        is_error: None,
                        content: anthropic::types::ToolResultContent::Text(truncated_output),
                    });
                }
            }
        }

        // 5. Append results and continue the loop
        history.push(Message {
            role: Role::User,
            content: results,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cwd() {
        let cwd = get_cwd();
        assert!(!cwd.is_empty());
        assert_ne!(cwd, ".");
    }

    #[test]
    fn test_get_bash_tool() {
        let tool = get_bash_tool();
        assert_eq!(tool.name, "bash");
        assert!(tool.description.contains("Execute shell command"));
        assert!(tool.description.contains("Subagent"));
    }

    #[test]
    fn test_get_system_prompt() {
        let prompt = get_system_prompt();
        assert!(prompt.contains("CLI agent"));
        assert!(prompt.contains("bash commands"));
        assert!(prompt.contains("subagent"));
    }

    #[test]
    fn test_execute_bash_simple_command() {
        let result = execute_bash("echo 'Hello, World!'");
        assert!(result.contains("Hello, World!"));
    }

    #[test]
    fn test_execute_bash_with_newline() {
        let result = execute_bash("echo -e 'Line1\\nLine2'");
        assert!(result.contains("Line1"));
        assert!(result.contains("Line2"));
    }

    #[test]
    fn test_execute_bash_pwd() {
        let result = execute_bash("pwd");
        assert!(!result.is_empty());
        // Should contain a valid path
        assert!(result.contains("/"));
    }

    #[test]
    fn test_execute_bash_error_command() {
        let result = execute_bash("nonexistent_command_12345");
        // Should contain error message
        assert!(result.contains("not found") || result.contains("command not found"));
    }

    #[test]
    fn test_execute_bash_with_pipe() {
        let result = execute_bash("echo 'test' | wc -l");
        assert!(result.trim().contains("1"));
    }

    #[test]
    fn test_execute_bash_ls() {
        let result = execute_bash("ls -la");
        // Should contain common directory entries
        assert!(result.contains(".") || result.contains("total"));
    }
}
