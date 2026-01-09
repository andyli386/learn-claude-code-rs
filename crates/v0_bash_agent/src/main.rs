#!/usr/bin/env rust
//! v0_bash_agent - Mini Claude Code: Bash is All You Need
//!
//! Core Philosophy: "Bash is All You Need"
//! ======================================
//! This is the ULTIMATE simplification of a coding agent. After building v1-v3,
//! we ask: what is the ESSENCE of an agent?
//!
//! The answer: ONE tool (bash) + ONE loop = FULL agent capability.
//!
//! Why Bash is Enough:
//! ------------------
//! Unix philosophy says everything is a file, everything can be piped.
//! Bash is the gateway to this world:
//!
//!     | You need      | Bash command                           |
//!     |---------------|----------------------------------------|
//!     | Read files    | cat, head, tail, grep                  |
//!     | Write files   | echo '...' > file, cat << 'EOF' > file |
//!     | Search        | find, grep, rg, ls                     |
//!     | Execute       | cargo, npm, make, any command          |
//!     | **Subagent**  | v0_bash_agent "task"                   |
//!
//! The last line is the KEY INSIGHT: calling itself via bash implements subagents!
//! No Task tool, no Agent Registry - just recursion through process spawning.
//!
//! How Subagents Work:
//! ------------------
//!     Main Agent
//!       |-- bash: v0_bash_agent "analyze architecture"
//!            |-- Subagent (isolated process, fresh history)
//!                 |-- bash: find . -name "*.rs"
//!                 |-- bash: cat src/main.rs
//!                 |-- Returns summary via stdout
//!
//! Process isolation = Context isolation:
//! - Child process has its own history=[]
//! - Parent captures stdout as tool result
//! - Recursive calls enable unlimited nesting
//!
//! Usage:
//!     # Interactive mode
//!     v0_bash_agent
//!
//!     # Subagent mode (called by parent agent or directly)
//!     v0_bash_agent "explore src/ and summarize"

use anthropic::Client;
use anyhow::Result;
use colored::*;
use std::env;
use std::io::{self, Write};
use v0_bash_agent::chat;

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

/// Get model name from environment or use default (Sonnet)
fn get_model_name() -> String {
    env::var("MODEL_NAME").unwrap_or_else(|_| "claude-sonnet-4-5-20250929".to_string())
}

/// Map model alias to full model name
fn resolve_model_alias(alias: &str) -> Option<String> {
    match alias.to_lowercase().as_str() {
        "sonnet" => Some("claude-sonnet-4-5-20250929".to_string()),
        "opus" => Some("claude-opus-4-5-20251101".to_string()),
        _ => None,
    }
}

/// Parse command line arguments
/// Returns: (model_name, optional_task)
fn parse_args() -> (String, Option<String>) {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        // No arguments: interactive mode with default model
        return (get_model_name(), None);
    }

    // Check if first argument is a model alias
    if let Some(model) = resolve_model_alias(&args[1]) {
        if args.len() > 2 {
            // Model + task: subagent mode with specified model
            let task = args[2..].join(" ");
            return (model, Some(task));
        } else {
            // Just model: interactive mode with specified model
            return (model, None);
        }
    }

    // First argument is not a model alias, treat as task
    let task = args[1..].join(" ");
    (get_model_name(), Some(task))
}

/// Print model selection info
fn print_model_info(model: &str) {
    let model_name = if model.contains("sonnet") {
        "Claude 4.5 Sonnet (faster)"
    } else if model.contains("opus") {
        "Claude 4.5 Opus (more capable)"
    } else {
        "Custom model"
    };

    println!(
        "{}",
        format!("🤖 Using model: {} ({})", model_name, model).bright_blue()
    );
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    let client = create_client()?;
    let (model, task) = parse_args();

    if let Some(task) = task {
        // Subagent mode: execute task and print result
        // This is how parent agents spawn children via bash
        let mut history = vec![];
        let result = chat(&client, &model, &task, &mut history).await?;
        println!("{}", result);
    } else {
        // Interactive REPL mode
        print_model_info(&model);
        println!(
            "{}",
            "Type 'q' or 'exit' to quit. Type 'help' for usage examples.\n".bright_black()
        );

        let mut history = vec![];
        loop {
            print!("{}", ">> ".cyan());
            io::stdout().flush()?;

            let mut query = String::new();
            if io::stdin().read_line(&mut query).is_err() {
                break;
            }

            let query = query.trim();
            if query.is_empty() || query == "q" || query == "exit" {
                break;
            }

            if query == "help" {
                print_help();
                continue;
            }

            match chat(&client, &model, query, &mut history).await {
                Ok(response) => println!("{}", response),
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    }

    Ok(())
}

/// Print help message
fn print_help() {
    println!("{}", "Usage Examples:".bright_green());
    println!("  {}", "Basic commands:".bright_yellow());
    println!("    >> ls -la");
    println!("    >> cat README.md");
    println!("    >> find . -name '*.rs'");
    println!();
    println!("  {}", "File operations:".bright_yellow());
    println!("    >> echo 'Hello' > test.txt");
    println!("    >> cat test.txt");
    println!();
    println!("  {}", "Spawn subagent for complex tasks:".bright_yellow());
    println!("    >> v0_bash_agent 'analyze all rust files'");
    println!("    >> v0_bash_agent opus 'review code quality'");
    println!();
}
