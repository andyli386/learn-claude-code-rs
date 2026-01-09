#!/usr/bin/env rust
//! Direct SDK test to isolate the issue

use anthropic::types::{ContentBlock, Message, MessagesRequestBuilder, Role, Tool};
use anyhow::Result;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    println!("=== Direct SDK Test ===\n");

    let api_key = env::var("ANTHROPIC_API_KEY").or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))?;

    let base_url = env::var("ANTHROPIC_API_BASE")
        .or_else(|_| env::var("ANTHROPIC_BASE_URL"))
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

    let model = env::var("MODEL_NAME").unwrap_or_else(|_| "claude-sonnet-4-5-20250929".to_string());

    println!("Base URL: {}", base_url);
    println!("Model: {}\n", model);

    let client = anthropic::client::ClientBuilder::new()
        .api_key(api_key)
        .api_base(base_url)
        .build()?;

    // Test 1: Simple message without tools
    println!("Test 1: Simple message (no tools)");
    println!("=====================================");
    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text("Say hello in one word")],
    }];

    let request = MessagesRequestBuilder::new(model.clone(), messages, 128).build()?;

    match client.messages(request).await {
        Ok(response) => {
            println!("✓ Success!");
            println!("Response: {:?}\n", response.content);
        }
        Err(e) => {
            println!("✗ Failed: {}", e);
            println!("Error: {:?}\n", e);
            return Err(e.into());
        }
    }

    // Test 2: Message with bash tool
    println!("Test 2: Message with bash tool");
    println!("=====================================");
    let bash_tool = Tool {
        name: "bash".to_string(),
        description: "Execute shell command".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"}
            },
            "required": ["command"]
        }),
    };

    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text("Run 'echo hello' using bash")],
    }];

    let request = MessagesRequestBuilder::new(model.clone(), messages, 1000)
        .tools(vec![bash_tool])
        .build()?;

    match client.messages(request).await {
        Ok(response) => {
            println!("✓ Success!");
            println!("Stop reason: {:?}", response.stop_reason);
            println!("Content blocks: {}", response.content.len());
            for (i, block) in response.content.iter().enumerate() {
                println!("  Block {}: {:?}", i, block);
            }
        }
        Err(e) => {
            println!("✗ Failed: {}", e);
            println!("Error: {:?}", e);
            return Err(e.into());
        }
    }

    println!("\n=== All Tests Passed! ===");
    Ok(())
}
