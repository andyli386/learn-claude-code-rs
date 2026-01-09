#!/usr/bin/env rust
//! Tool to test different model names and find which ones work

use anyhow::Result;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    println!("=== Testing Available Models ===\n");

    let api_key = env::var("ANTHROPIC_API_KEY")
        .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))
        .map_err(|_| anyhow::anyhow!("Missing API key"))?;

    let base_url = env::var("ANTHROPIC_API_BASE")
        .or_else(|_| env::var("ANTHROPIC_BASE_URL"))
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

    println!("Base URL: {}\n", base_url);

    // Claude 4.5 models to test
    let models = vec![
        "claude-sonnet-4-5-20250929",
        "claude-opus-4-5-20251101",
    ];

    let client = reqwest::Client::new();
    let url = format!("{}/v1/messages", base_url.trim_end_matches('/'));

    for model in models {
        print!("Testing {}... ", model);

        let payload = json!({
            "model": model,
            "max_tokens": 10,
            "messages": [{
                "role": "user",
                "content": "Hi"
            }]
        });

        let response = client
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if status.is_success() {
            println!("✓ WORKS!");
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(content) = json.get("content") {
                    println!("  Response: {:?}", content);
                }
            }
        } else {
            print!("✗ FAILED ({})", status);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(error) = json.get("error") {
                    if let Some(msg) = error.get("message") {
                        println!(" - {}", msg.as_str().unwrap_or("Unknown error"));
                    } else {
                        println!();
                    }
                } else {
                    println!();
                }
            } else {
                println!();
            }
        }
    }

    println!("\n=== Recommendation ===");
    println!("Add the working model name to your .env file:");
    println!("MODEL_NAME=<working_model_name>");

    Ok(())
}
