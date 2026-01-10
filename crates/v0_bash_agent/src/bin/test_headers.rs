#!/usr/bin/env rust
//! Tool to test different header combinations to bypass NewAPI restrictions

use anyhow::Result;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let api_key = env::var("ANTHROPIC_API_KEY").or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))?;
    let base_url = env::var("ANTHROPIC_API_BASE")
        .or_else(|_| env::var("ANTHROPIC_BASE_URL"))
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
    let model = "claude-sonnet-4-5-20250929";

    println!("=== Testing Different Header Combinations ===\n");

    let test_cases = [
        ("claude-code/2.1.2", Some("claude-code/2.1.2")),
        ("claude-code/2.1.2", Some("Claude Code/2.1.2")),
        ("claude-code/2.1.2", None),
        ("Claude Code/2.1.2", Some("claude-code/2.1.2")),
        ("anthropic-sdk-typescript/0.32.1", Some("claude-code/2.1.2")),
    ];

    for (i, (user_agent, origin)) in test_cases.iter().enumerate() {
        println!(
            "Test {}: User-Agent: {:?}, Origin: {:?}",
            i + 1,
            user_agent,
            origin
        );

        let client = reqwest::Client::new();
        let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

        let payload = json!({
            "model": model,
            "max_tokens": 10,
            "messages": [{
                "role": "user",
                "content": "Hi"
            }]
        });

        let mut request_builder = client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("user-agent", *user_agent);

        if let Some(origin_val) = origin {
            request_builder = request_builder.header("origin", *origin_val);
        }

        let response = request_builder.json(&payload).send().await?;

        let status = response.status();
        let body = response.text().await?;

        print!("  Status: {} ", status);
        if status.is_success() {
            println!("✓ SUCCESS!");
            println!("  Response: {}\n", &body[..body.len().min(100)]);
        } else {
            println!("✗ FAILED");
            if body.contains("only authorized for use with Claude Code") {
                println!("  Error: Claude Code restriction detected");
            } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(error) = json.get("error") {
                    println!("  Error: {}", error);
                }
            }
            println!();
        }
    }

    println!("\n=== Recommendation ===");
    println!("Check the test results above to see which header combination works.");
    println!("If none work, NewAPI may be using additional validation beyond headers.");

    Ok(())
}
