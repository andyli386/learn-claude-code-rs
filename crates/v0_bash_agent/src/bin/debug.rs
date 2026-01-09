#!/usr/bin/env rust
//! Debug tool to test API connection and see raw responses

use anyhow::Result;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    println!("=== Debug: API Connection Test ===\n");

    // Try to get API key
    let api_key = env::var("ANTHROPIC_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))
        .map_err(|_| anyhow::anyhow!("Missing API key"))?;

    println!("✓ API Key found: {}...", &api_key[..20.min(api_key.len())]);

    // Try to get base URL
    let base_url = env::var("ANTHROPIC_API_BASE")
        .or_else(|_| env::var("ANTHROPIC_BASE_URL"))
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

    println!("✓ Base URL: {}", base_url);

    // Get model
    let model = env::var("MODEL_NAME").unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
    println!("✓ Model: {}", model);

    println!("\n=== Testing Raw HTTP Request ===\n");

    // Make a raw HTTP request to see what we get back
    use serde_json::json;

    let client = reqwest::Client::new();
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    println!("URL: {}", url);

    let payload = json!({
        "model": model,
        "max_tokens": 128,
        "messages": [{
            "role": "user",
            "content": "Say hello in one word"
        }]
    });

    println!("Payload: {}", serde_json::to_string_pretty(&payload)?);

    let response = client
        .post(&url)
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await?;

    println!("\n=== Response ===");
    println!("Status: {}", response.status());
    println!("Headers:");
    for (key, value) in response.headers() {
        println!("  {}: {:?}", key, value);
    }

    let body = response.text().await?;
    println!("\nBody (first 1000 chars):");
    println!("{}", &body[..body.len().min(1000)]);

    if body.len() > 1000 {
        println!("... (truncated, total {} bytes)", body.len());
    }

    // Try to parse as JSON
    println!("\n=== Parsing as JSON ===");
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(json) => {
            println!("✓ Valid JSON:");
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        Err(e) => {
            println!("✗ Not valid JSON: {}", e);
            println!("\nThis might be HTML or plain text. First 500 chars:");
            println!("{}", &body[..body.len().min(500)]);
        }
    }

    Ok(())
}
