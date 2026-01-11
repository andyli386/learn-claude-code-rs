//! MCP Browser Client Module
//!
//! Provides integration with chrome-devtools-mcp for browser automation

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// MCP Browser client for controlling Chrome/Edge
pub struct McpBrowserClient {
    process: Arc<Mutex<Option<std::process::Child>>>,
    request_id: Arc<Mutex<u64>>,
}

impl McpBrowserClient {
    /// Create a new MCP browser client
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            request_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Check if chrome-devtools-mcp is available
    pub fn check_available(&self) -> bool {
        let output = Command::new("npx")
            .args(["-y", "chrome-devtools-mcp@latest", "--version"])
            .output();

        match output {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// Start the MCP server process
    pub fn start(&self) -> Result<()> {
        let mut process_guard = self
            .process
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock process: {}", e))?;

        if process_guard.is_some() {
            return Ok(()); // Already started
        }

        println!("🌐 Starting chrome-devtools-mcp server...");

        // Start chrome-devtools-mcp process with Chrome remote debugging URL
        let mcp_process = Command::new("npx")
            .args(["-y", "chrome-devtools-mcp@latest"])
            .env("CHROME_REMOTE_DEBUGGING_URL", "http://localhost:9222")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start chrome-devtools-mcp. Make sure Node.js is installed")?;

        *process_guard = Some(mcp_process);

        // Give it time to start
        thread::sleep(Duration::from_secs(2));

        println!("✅ chrome-devtools-mcp server started");

        Ok(())
    }

    /// Send a JSON-RPC request to the MCP server
    fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let request_id = {
            let mut id = self
                .request_id
                .lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock request_id: {}", e))?;
            *id += 1;
            *id
        };

        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": request_id
        });

        let request_str = serde_json::to_string(&request)?;
        let response = self.execute_request(&request_str)?;

        Ok(response)
    }

    /// Execute a request and get response
    fn execute_request(&self, request: &str) -> Result<Value> {
        let mut process_guard = self
            .process
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock process: {}", e))?;

        let process = process_guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP process not started"))?;

        // Send request
        if let Some(stdin) = process.stdin.as_mut() {
            writeln!(stdin, "{}", request).context("Failed to write to MCP stdin")?;
        }

        // Read response
        if let Some(stdout) = process.stdout.as_mut() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = line?;
                if line.trim().starts_with('{') {
                    if let Ok(response) = serde_json::from_str::<Value>(&line) {
                        return Ok(response);
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No valid response from MCP server"))
    }

    /// Navigate to a URL
    pub fn navigate(&self, url: &str) -> Result<String> {
        let response = self.send_request(
            "tools/call",
            json!({
                "name": "navigate_page",
                "arguments": {
                    "type": "url",
                    "url": url
                }
            }),
        )?;

        extract_result_text(&response)
    }

    /// Take a screenshot
    pub fn screenshot(&self) -> Result<String> {
        let response = self.send_request(
            "tools/call",
            json!({
                "name": "take_screenshot",
                "arguments": {}
            }),
        )?;

        extract_result_text(&response)
    }

    /// Get performance metrics
    pub fn get_performance(&self) -> Result<String> {
        // Start trace with auto-stop
        let _start = self.send_request(
            "tools/call",
            json!({
                "name": "performance_start_trace",
                "arguments": {
                    "reload": false,
                    "autoStop": true
                }
            }),
        )?;

        // Wait for trace to complete
        thread::sleep(Duration::from_secs(3));

        // Stop trace and get results
        let response = self.send_request(
            "tools/call",
            json!({
                "name": "performance_stop_trace",
                "arguments": {}
            }),
        )?;

        extract_result_text(&response)
    }

    /// Get network requests
    pub fn get_network(&self) -> Result<String> {
        let response = self.send_request(
            "tools/call",
            json!({
                "name": "list_network_requests",
                "arguments": {}
            }),
        )?;

        extract_result_text(&response)
    }

    /// Click an element (requires uid from snapshot)
    pub fn click(&self, uid: &str) -> Result<String> {
        let response = self.send_request(
            "tools/call",
            json!({
                "name": "click",
                "arguments": {
                    "uid": uid
                }
            }),
        )?;

        extract_result_text(&response)
    }

    /// Take a text snapshot of the page
    pub fn take_snapshot(&self) -> Result<String> {
        let response = self.send_request(
            "tools/call",
            json!({
                "name": "take_snapshot",
                "arguments": {
                    "verbose": false
                }
            }),
        )?;

        extract_result_text(&response)
    }

    /// Stop the MCP server
    pub fn stop(&self) -> Result<()> {
        let mut process_guard = self
            .process
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock process: {}", e))?;

        if let Some(mut process) = process_guard.take() {
            process.kill().context("Failed to kill MCP process")?;
            println!("🛑 chrome-devtools-mcp server stopped");
        }

        Ok(())
    }
}

impl Drop for McpBrowserClient {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Extract text content from MCP response
fn extract_result_text(response: &Value) -> Result<String> {
    if let Some(result) = response.get("result") {
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            for item in content {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }
    }

    if let Some(error) = response.get("error") {
        anyhow::bail!("MCP error: {}", error);
    }

    Ok("Operation completed".to_string())
}

/// Create browser automation tools for Claude
#[allow(dead_code)]
pub fn create_browser_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "browser_navigate",
            "description": "Navigate to a URL in Chrome/Edge browser",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to navigate to"
                    }
                },
                "required": ["url"]
            }
        }),
        json!({
            "name": "browser_screenshot",
            "description": "Take a screenshot of the current browser page",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "browser_get_performance",
            "description": "Get performance metrics from Chrome DevTools",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "browser_get_network",
            "description": "Analyze network requests made by the page",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}
