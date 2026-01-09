#!/usr/bin/env rust
//! Compare SDK request with manual request to find differences

use anyhow::Result;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let api_key = env::var("ANTHROPIC_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))?;
    let base_url = env::var("ANTHROPIC_API_BASE")
        .or_else(|_| env::var("ANTHROPIC_BASE_URL"))
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
    let model = "claude-sonnet-4-5-20250929";

    println!("=== Comparing Requests with Tools ===\n");

    // Test 1: Manual reqwest with tools (same as test_headers)
    println!("Test 1: Manual reqwest with bash tool");
    println!("======================================");

    let client = reqwest::Client::new();
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    let payload = json!({
        "model": model,
        "max_tokens": 1000,
        "messages": [{
            "role": "user",
            "content": "Run echo hello using bash"
        }],
        "tools": [{
            "name": "bash",
            "description": "Execute shell command",
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"]
            }
        }]
    });

    println!("Request payload:");
    println!("{}\n", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&url)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("user-agent", "claude-code/2.1.2")
        .json(&payload)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    println!("Response status: {}", status);
    if status.is_success() {
        println!("✓ SUCCESS!");
        println!("Response: {}\n", &body[..body.len().min(200)]);
    } else {
        println!("✗ FAILED");
        println!("Error: {}\n", &body[..body.len().min(500)]);
    }

    // Test 2: Try without tools but with tool_choice (if Claude Code uses it)
    println!("\nTest 2: Without tools field");
    println!("===========================");

    let payload_no_tools = json!({
        "model": model,
        "max_tokens": 1000,
        "messages": [{
            "role": "user",
            "content": "Say hello"
        }]
    });

    let response2 = client
        .post(&url)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("user-agent", "claude-code/2.1.2")
        .json(&payload_no_tools)
        .send()
        .await?;

    let status2 = response2.status();
    println!("Response status: {}", status2);
    println!("Result: {}", if status2.is_success() { "✓ SUCCESS" } else { "✗ FAILED" });

    println!("\n=== Analysis ===");
    println!("If Test 1 fails but Test 2 succeeds, NewAPI restricts tools usage.");
    println!("This credential may only support simple messages, not agentic tool use.");

    Ok(())
}
