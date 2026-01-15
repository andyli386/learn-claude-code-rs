//! v4_skills_agent - Mini Claude Code: Skills Mechanism (~1500 lines)
//!
//! Core Philosophy: "Knowledge Externalization"
//! ============================================
//! v3 gave us subagents for task decomposition. But there's a deeper question:
//!
//!     How does the model know HOW to handle domain-specific tasks?
//!
//! - Processing PDFs? It needs to know pdftotext vs PyMuPDF
//! - Building MCP servers? It needs protocol specs and best practices
//! - Code review? It needs a systematic checklist
//!
//! This knowledge isn't a tool - it's EXPERTISE. Skills solve this by letting
//! the model load domain knowledge on-demand.
//!
//! The Paradigm Shift: Knowledge Externalization
//! ---------------------------------------------
//! Traditional AI: Knowledge locked in model parameters
//!   - To teach new skills: collect data -> train -> deploy
//!   - Cost: $10K-$1M+, Timeline: Weeks
//!   - Requires ML expertise, GPU clusters
//!
//! Skills: Knowledge stored in editable files
//!   - To teach new skills: write a SKILL.md file
//!   - Cost: Free, Timeline: Minutes
//!   - Anyone can do it
//!
//! It's like attaching a hot-swappable LoRA adapter without any training!
//!
//! Tools vs Skills:
//! ----------------
//!     | Concept   | What it is              | Example                    |
//!     |-----------|-------------------------|----------------------------|
//!     | **Tool**  | What model CAN do       | bash, read_file, write     |
//!     | **Skill** | How model KNOWS to do   | PDF processing, MCP dev    |
//!
//! Tools are capabilities. Skills are knowledge.
//!
//! Progressive Disclosure:
//! -----------------------
//!     Layer 1: Metadata (always loaded)      ~100 tokens/skill
//!              name + description only
//!
//!     Layer 2: SKILL.md body (on trigger)    ~2000 tokens
//!              Detailed instructions
//!
//!     Layer 3: Resources (as needed)         Unlimited
//!              scripts/, references/, assets/
//!
//! This keeps context lean while allowing arbitrary depth.
//!
//! SKILL.md Standard:
//! ------------------
//!     skills/
//!     |-- pdf/
//!     |   |-- SKILL.md          # Required: YAML frontmatter + Markdown body
//!     |-- mcp-builder/
//!     |   |-- SKILL.md
//!     |   |-- references/       # Optional: docs, specs
//!     |-- code-review/
//!         |-- SKILL.md
//!         |-- scripts/          # Optional: helper scripts
//!
//! Cache-Preserving Injection:
//! ---------------------------
//! Critical insight: Skill content goes into tool_result (user message),
//! NOT system prompt. This preserves prompt cache!
//!
//!     Wrong: Edit system prompt each time (cache invalidated, 20-50x cost)
//!     Right: Append skill as tool result (prefix unchanged, cache hit)
//!
//! This is how production Claude Code works - and why it's cost-efficient.
//!
//! Usage:
//!     cargo run -p v4_skills_agent

use anthropic::types::{
    ContentBlock, Message, MessagesRequestBuilder, Role, StopReason, SystemPrompt, Tool,
};
use anthropic::Client;
use anyhow::{Context, Result};
use colored::Colorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
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
// Thinking Animation (from v2/v3)
// =============================================================================

fn spawn_thinking_animation() -> ThinkingAnimation {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let handle = thread::spawn(move || {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut idx = 0;

        print!("\x1B[?25l");
        io::stdout().flush().ok();

        while running_clone.load(Ordering::Relaxed) {
            let frame = frames[idx % frames.len()];
            print!("\r{} {}...", frame.bright_cyan(), "Thinking".bright_black());
            io::stdout().flush().ok();

            thread::sleep(Duration::from_millis(80));
            idx += 1;
        }

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
    skills_dir: PathBuf,
    max_output_tokens: u32,
    max_truncation_retries: usize,
}

impl Config {
    fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let model =
            env::var("MODEL_NAME").unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
        let workdir = env::current_dir().context("Failed to get current directory")?;
        let skills_dir = workdir.join("skills");

        let max_output_tokens = env::var("MINI_CODE_MAX_OUTPUT_TOKENS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(160000)
            .clamp(1000, 100_000_000);

        let max_truncation_retries = env::var("MINI_CODE_MAX_TRUNCATION_RETRIES")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3)
            .clamp(1, 10);

        Ok(Self {
            model,
            workdir,
            skills_dir,
            max_output_tokens,
            max_truncation_retries,
        })
    }

    fn system_prompt(&self, skill_descriptions: &str, agent_descriptions: &str) -> String {
        format!(
            r#"You are a coding agent at {}.

Loop: plan -> act with tools -> report.

**Skills available** (invoke with Skill tool when task matches):
{}

**Subagents available** (invoke with Task tool for focused subtasks):
{}

Rules:
- Use Skill tool IMMEDIATELY when a task matches a skill description
- Use Task tool for subtasks needing focused exploration or implementation
- Use TodoWrite to track multi-step work
- Prefer tools over prose. Act, don't just explain.
- After finishing, summarize what changed."#,
            self.workdir.display(),
            skill_descriptions,
            agent_descriptions
        )
    }
}

// =============================================================================
// SkillLoader - The core addition in v4
// =============================================================================

#[derive(Debug, Clone)]
struct Skill {
    name: String,
    description: String,
    body: String,
    dir: PathBuf,
}

/// Loads and manages skills from SKILL.md files.
///
/// A skill is a FOLDER containing:
/// - SKILL.md (required): YAML frontmatter + markdown instructions
/// - scripts/ (optional): Helper scripts the model can run
/// - references/ (optional): Additional documentation
/// - assets/ (optional): Templates, files for output
///
/// SKILL.md Format:
/// ----------------
///     ---
///     name: pdf
///     description: Process PDF files. Use when reading, creating, or merging PDFs.
///     ---
///
///     # PDF Processing Skill
///
///     ## Reading PDFs
///
///     Use pdftotext for quick extraction:
///     ```bash
///     pdftotext input.pdf -
///     ```
///     ...
///
/// The YAML frontmatter provides metadata (name, description).
/// The markdown body provides detailed instructions.
struct SkillLoader {
    skills: HashMap<String, Skill>,
}

impl SkillLoader {
    fn new(skills_dir: &Path) -> Self {
        let mut loader = Self {
            skills: HashMap::new(),
        };
        loader.load_skills(skills_dir);
        loader
    }

    /// Parse a SKILL.md file into metadata and body.
    ///
    /// Returns Some(Skill) if valid, None otherwise.
    fn parse_skill_md(&self, path: &Path) -> Option<Skill> {
        let content = fs::read_to_string(path).ok()?;

        // Match YAML frontmatter between --- markers
        let re = Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n(.*)$").ok()?;
        let caps = re.captures(&content)?;

        let frontmatter = caps.get(1)?.as_str();
        let body = caps.get(2)?.as_str();

        // Parse YAML-like frontmatter (simple key: value)
        let mut metadata = HashMap::new();
        for line in frontmatter.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim();
                let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
                metadata.insert(key.to_string(), value.to_string());
            }
        }

        // Require name and description
        let name = metadata.get("name")?.clone();
        let description = metadata.get("description")?.clone();

        Some(Skill {
            name,
            description,
            body: body.trim().to_string(),
            dir: path.parent()?.to_path_buf(),
        })
    }

    /// Scan skills directory and load all valid SKILL.md files.
    ///
    /// Only loads metadata at startup - body is already loaded but
    /// only sent to model when Skill tool is invoked.
    /// This keeps the initial system prompt lean.
    fn load_skills(&mut self, skills_dir: &Path) {
        if !skills_dir.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(skills_dir) {
            for entry in entries.flatten() {
                if !entry.path().is_dir() {
                    continue;
                }

                let skill_md = entry.path().join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }

                if let Some(skill) = self.parse_skill_md(&skill_md) {
                    self.skills.insert(skill.name.clone(), skill);
                }
            }
        }
    }

    /// Generate skill descriptions for system prompt.
    ///
    /// This is Layer 1 - only name and description, ~100 tokens per skill.
    /// Full content (Layer 2) is loaded only when Skill tool is called.
    fn get_descriptions(&self) -> String {
        if self.skills.is_empty() {
            return "(no skills available)".to_string();
        }

        self.skills
            .values()
            .map(|skill| format!("- {}: {}", skill.name, skill.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get full skill content for injection.
    ///
    /// This is Layer 2 - the complete SKILL.md body, plus any available
    /// resources (Layer 3 hints).
    ///
    /// Returns None if skill not found.
    fn get_skill_content(&self, name: &str) -> Option<String> {
        let skill = self.skills.get(name)?;

        let mut content = format!("# Skill: {}\n\n{}", skill.name, skill.body);

        // List available resources (Layer 3 hints)
        let mut resources = Vec::new();
        for (folder, label) in [
            ("scripts", "Scripts"),
            ("references", "References"),
            ("assets", "Assets"),
        ] {
            let folder_path = skill.dir.join(folder);
            if folder_path.exists() {
                if let Ok(entries) = fs::read_dir(&folder_path) {
                    let files: Vec<_> = entries
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    if !files.is_empty() {
                        resources.push(format!("{}: {}", label, files.join(", ")));
                    }
                }
            }
        }

        if !resources.is_empty() {
            content.push_str(&format!(
                "\n\n**Available resources in {}:**\n",
                skill.dir.display()
            ));
            for r in resources {
                content.push_str(&format!("- {}\n", r));
            }
        }

        Some(content)
    }

    fn list_skills(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }
}

// =============================================================================
// Agent Type Registry (from v3)
// =============================================================================

#[derive(Debug, Clone)]
struct AgentConfig {
    description: String,
    tools: Vec<String>,
    prompt: String,
}

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
            tools: vec!["*".to_string()],
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

fn get_agent_descriptions() -> String {
    let types = get_agent_types();
    types
        .iter()
        .map(|(name, cfg)| format!("- {}: {}", name, cfg.description))
        .collect::<Vec<_>>()
        .join("\n")
}

// =============================================================================
// TodoManager (from v2)
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

struct TodoManager {
    items: Arc<Mutex<Vec<TodoItem>>>,
}

impl TodoManager {
    fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn update(&self, new_items: Vec<TodoItem>) -> Result<String> {
        let mut in_progress_count = 0;

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

        if new_items.len() > 20 {
            anyhow::bail!("Max 20 todos allowed");
        }
        if in_progress_count > 1 {
            anyhow::bail!("Only one task can be in_progress at a time");
        }

        *self.items.lock().unwrap() = new_items;

        Ok(self.render())
    }

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
// Web Search Tool (from ai-research-agent)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Perform web search using DuckDuckGo HTML scraping.
async fn web_search(query: &str, max_results: usize) -> Result<Vec<SearchResult>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()?;

    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );

    // Add small delay to avoid rate limiting
    tokio::time::sleep(Duration::from_millis(500)).await;

    let response = client.get(&url).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Search failed with status: {}", response.status());
    }

    let body = response.text().await?;
    let results = parse_search_html(&body, max_results);

    Ok(results)
}

/// Parse DuckDuckGo HTML to extract search results.
fn parse_search_html(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut seen_urls = HashSet::new();

    // Strategy 1: Look for result links with the uddg parameter (redirect URLs)
    for segment in html.split("uddg=") {
        if results.len() >= max_results {
            break;
        }

        // Find the end of the encoded URL
        if let Some(end) = segment.find(['&', '"', '\'']) {
            let encoded_url = &segment[..end];
            if let Ok(url) = urlencoding::decode(encoded_url) {
                let url_str = url.to_string();
                if url_str.starts_with("http")
                    && !url_str.contains("duckduckgo.com")
                    && !seen_urls.contains(&url_str)
                {
                    seen_urls.insert(url_str.clone());
                    results.push(SearchResult {
                        title: extract_domain(&url_str).unwrap_or_else(|| "Result".to_string()),
                        url: url_str,
                        snippet: "Search result from DuckDuckGo".to_string(),
                    });
                }
            }
        }
    }

    // Strategy 2: Look for result__url class which contains visible URLs
    if results.len() < max_results {
        for segment in html.split("result__url") {
            if results.len() >= max_results {
                break;
            }

            // Look for href after this marker
            if let Some(href_start) = segment.find("href=\"") {
                let after_href = &segment[href_start + 6..];
                if let Some(href_end) = after_href.find('"') {
                    let href = &after_href[..href_end];
                    let url = if href.starts_with("//") {
                        format!("https:{}", href)
                    } else if href.starts_with("http") {
                        href.to_string()
                    } else {
                        continue;
                    };

                    if !url.contains("duckduckgo.com") && !seen_urls.contains(&url) {
                        seen_urls.insert(url.clone());
                        results.push(SearchResult {
                            title: extract_domain(&url).unwrap_or_else(|| "Result".to_string()),
                            url,
                            snippet: "Search result".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Strategy 3: Direct URL extraction - find any https:// URLs
    if results.len() < max_results {
        for segment in html.split("https://") {
            if results.len() >= max_results {
                break;
            }

            if let Some(end) = segment.find(|c: char| {
                c == '"' || c == '\'' || c == '<' || c == '>' || c == ' ' || c == ')'
            }) {
                let domain_path = &segment[..end];
                // Filter out internal/tracking URLs
                if !domain_path.starts_with("duckduckgo")
                    && !domain_path.starts_with("improving.duckduckgo")
                    && !domain_path.contains("cdn.")
                    && !domain_path.contains(".js")
                    && !domain_path.contains(".css")
                    && !domain_path.contains(".png")
                    && !domain_path.contains(".ico")
                    && domain_path.contains('.')
                    && domain_path.len() > 5
                {
                    let url = format!("https://{}", domain_path);
                    if !seen_urls.contains(&url) {
                        seen_urls.insert(url.clone());
                        results.push(SearchResult {
                            title: extract_domain(&url).unwrap_or_else(|| "Result".to_string()),
                            url,
                            snippet: "Search result".to_string(),
                        });
                    }
                }
            }
        }
    }

    results.into_iter().take(max_results).collect()
}

/// Extract the domain name from a URL.
fn extract_domain(url: &str) -> Option<String> {
    url.split("//")
        .nth(1)?
        .split('/')
        .next()
        .map(|s| s.to_string())
}

// =============================================================================
// Tool Definitions
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
            name: "web_search".to_string(),
            description: "Search the web using DuckDuckGo. Use this to find current information about any topic.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to find information about"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 5)",
                        "minimum": 1,
                        "maximum": 10
                    }
                },
                "required": ["query"]
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

/// Create the Task tool (from v3)
fn create_task_tool() -> Tool {
    let agent_types = get_agent_types();
    let type_names: Vec<String> = agent_types.keys().cloned().collect();

    Tool {
        name: "Task".to_string(),
        description: format!(
            r#"Spawn a subagent for a focused subtask.

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

/// Create the Skill tool - NEW in v4
fn create_skill_tool(skill_loader: &SkillLoader) -> Tool {
    Tool {
        name: "Skill".to_string(),
        description: format!(
            r#"Load a skill to gain specialized knowledge for a task.

Available skills:
{}

When to use:
- IMMEDIATELY when user task matches a skill description
- Before attempting domain-specific work (PDF, MCP, etc.)

The skill content will be injected into the conversation, giving you
detailed instructions and access to resources."#,
            skill_loader.get_descriptions()
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "Name of the skill to load"
                }
            },
            "required": ["skill"]
        }),
    }
}

/// Get all tools for main agent (includes Task and Skill)
fn create_all_tools(skill_loader: &SkillLoader) -> Vec<Tool> {
    let mut tools = create_base_tools();
    tools.push(create_task_tool());
    tools.push(create_skill_tool(skill_loader));
    tools
}

/// Filter tools based on agent type
/// Note: This does NOT include the Skill tool - that must be added separately
/// by calling with skill_loader if needed
fn get_tools_for_agent(agent_type: &str) -> Vec<Tool> {
    let agent_types = get_agent_types();
    let config = match agent_types.get(agent_type) {
        Some(cfg) => cfg,
        None => return Vec::new(),
    };

    let base_tools = create_base_tools();

    if config.tools.contains(&"*".to_string()) {
        return base_tools;
    }

    base_tools
        .into_iter()
        .filter(|t| config.tools.contains(&t.name))
        .collect()
}

/// Get tools for subagent, including Skill tool if agent type supports it
fn get_tools_for_subagent(agent_type: &str, skill_loader: &SkillLoader) -> Vec<Tool> {
    let mut tools = get_tools_for_agent(agent_type);

    // Add Skill tool for agent types that can benefit from domain knowledge
    // explore: read-only, can use skills for analysis patterns
    // code: full access, can use skills for implementation guidance
    // plan: read-only, can use skills for design patterns
    match agent_type {
        "explore" | "code" | "plan" => {
            tools.push(create_skill_tool(skill_loader));
        }
        _ => {
            // Other agent types don't get Skill tool
        }
    }

    tools
}

// =============================================================================
// Tool Implementations
// =============================================================================

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
        Ok(safe_path) => {
            // Read file as raw bytes first to handle non-UTF8 content gracefully
            match fs::read(&safe_path) {
                Ok(bytes) => {
                    // Check for non-UTF8 bytes
                    let has_invalid_utf8 = bytes.iter().any(|&b| b > 0x7F && !b.is_ascii());

                    let content = if has_invalid_utf8 {
                        // Use lossy conversion to handle binary data in log files
                        String::from_utf8_lossy(&bytes).to_string()
                    } else {
                        // Safe to use strict UTF-8 for pure ASCII files
                        match String::from_utf8(bytes) {
                            Ok(s) => s,
                            Err(e) => {
                                // Fallback to lossy conversion
                                String::from_utf8_lossy(e.as_bytes()).to_string()
                            }
                        }
                    };

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
                Err(e) => format!("Error reading file: {}", e),
            }
        }
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

/// Load a skill and inject it into the conversation - NEW in v4
///
/// This is the key mechanism:
/// 1. Get skill content (SKILL.md body + resource hints)
/// 2. Return it wrapped in <skill-loaded> tags
/// 3. Model receives this as tool_result (user message)
/// 4. Model now "knows" how to do the task
///
/// Why tool_result instead of system prompt?
/// - System prompt changes invalidate cache (20-50x cost increase)
/// - Tool results append to end (prefix unchanged, cache hit)
///
/// This is how production systems stay cost-efficient.
fn run_skill(skill_loader: &SkillLoader, skill_name: &str) -> String {
    match skill_loader.get_skill_content(skill_name) {
        Some(content) => {
            format!(
                r#"<skill-loaded name="{}">
{}
</skill-loaded>

Follow the instructions in the skill above to complete the user's task."#,
                skill_name, content
            )
        }
        None => {
            let available = skill_loader.list_skills().join(", ");
            let available = if available.is_empty() {
                "none".to_string()
            } else {
                available
            };
            format!(
                "Error: Unknown skill '{}'. Available: {}",
                skill_name, available
            )
        }
    }
}

// =============================================================================
// Subagent Progress Tracking (from v3)
// =============================================================================

struct SubagentProgress {
    tool_count: usize,
    current_tool: Option<String>,
    start_time: Instant,
}

impl SubagentProgress {
    fn new() -> Self {
        Self {
            tool_count: 0,
            current_tool: None,
            start_time: Instant::now(),
        }
    }
}

fn spawn_subagent_progress_updater(
    agent_type: String,
    description: String,
    progress: Arc<Mutex<SubagentProgress>>,
    stop_signal: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        println!();
        io::stdout().flush().ok();

        while !stop_signal.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(1000));

            if stop_signal.load(Ordering::Relaxed) {
                break;
            }

            let progress_guard = progress.lock().unwrap();
            let elapsed = progress_guard.start_time.elapsed().as_secs_f64();
            let tool_count = progress_guard.tool_count;
            let current_tool = progress_guard.current_tool.clone();
            drop(progress_guard);

            println!(
                "\x1B[1A\x1B[K  {} {} ... {} tools, {:.1}s",
                format!("[{}]", agent_type).bright_magenta(),
                description,
                tool_count,
                elapsed
            );

            if let Some(tool_info) = current_tool {
                println!(
                    "\x1B[K    {} {}",
                    "→".bright_blue(),
                    tool_info.bright_black()
                );
            } else {
                println!("\x1B[K");
            }

            print!("\x1B[1A");
            io::stdout().flush().ok();
        }
    })
}

// =============================================================================
// Subagent Execution (from v3, adapted for v4)
// =============================================================================

async fn run_task(
    client: &Client,
    config: &Config,
    todo_manager: &TodoManager,
    skill_loader: &SkillLoader,
    description: &str,
    prompt: &str,
    agent_type: &str,
) -> String {
    let agent_types = get_agent_types();
    let agent_config = match agent_types.get(agent_type) {
        Some(cfg) => cfg,
        None => return format!("Error: Unknown agent type '{}'", agent_type),
    };

    let sub_system = format!(
        r#"You are a {} subagent at {}.

{}

Complete the task and return a clear, concise summary."#,
        agent_type,
        config.workdir.display(),
        agent_config.prompt
    );

    // Get tools including Skill tool for subagent
    let sub_tools = get_tools_for_subagent(agent_type, skill_loader);

    let mut sub_messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::text(prompt)],
    }];

    let progress = Arc::new(Mutex::new(SubagentProgress::new()));
    let progress_clone = progress.clone();
    let stop_signal = Arc::new(AtomicBool::new(false));
    let stop_signal_clone = stop_signal.clone();

    println!(
        "  {} {}",
        format!("[{}]", agent_type).bright_magenta(),
        description
    );

    let updater = spawn_subagent_progress_updater(
        agent_type.to_string(),
        description.to_string(),
        progress_clone,
        stop_signal_clone,
    );

    let mut consecutive_truncations = 0;

    let result = loop {
        let request = MessagesRequestBuilder::new(&config.model, sub_messages.clone(), 8000)
            .system(SystemPrompt::Text(sub_system.clone()))
            .tools(sub_tools.clone())
            .build();

        let request = match request {
            Ok(r) => r,
            Err(e) => break format!("Error building request: {}", e),
        };

        let response = match client.messages(request).await {
            Ok(r) => r,
            Err(e) => break format!("Error calling API: {}", e),
        };

        match response.stop_reason {
            Some(StopReason::MaxTokens) => {
                consecutive_truncations += 1;

                if consecutive_truncations >= 2 {
                    let progress_guard = progress.lock().unwrap();
                    let elapsed = progress_guard.start_time.elapsed();
                    let tool_count = progress_guard.tool_count;
                    drop(progress_guard);

                    break format!(
                        "[ERROR] Subagent output truncated {} times ({} tools, {:.1}s). Task too complex.",
                        consecutive_truncations,
                        tool_count,
                        elapsed.as_secs_f64()
                    );
                }

                sub_messages.push(Message {
                    role: Role::Assistant,
                    content: response.content,
                });

                sub_messages.push(Message {
                    role: Role::User,
                    content: vec![ContentBlock::text(
                        "[SYSTEM: Response truncated. Provide a brief summary only (max 200 words).]"
                    )],
                });

                continue;
            }

            Some(StopReason::ToolUse) => {
                consecutive_truncations = 0;

                let mut results = Vec::new();
                for block in &response.content {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        {
                            let mut progress_guard = progress.lock().unwrap();
                            progress_guard.tool_count += 1;

                            let tool_display = match name.as_str() {
                                "bash" => {
                                    if let Some(cmd) = input.get("command").and_then(|v| v.as_str())
                                    {
                                        let short_cmd = if cmd.len() > 60 {
                                            format!("{}...", &cmd[..60])
                                        } else {
                                            cmd.to_string()
                                        };
                                        format!("bash: {}", short_cmd)
                                    } else {
                                        "bash".to_string()
                                    }
                                }
                                "read_file" => {
                                    if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                                        format!("read: {}", path)
                                    } else {
                                        "read_file".to_string()
                                    }
                                }
                                "write_file" => {
                                    if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                                        format!("write: {}", path)
                                    } else {
                                        "write_file".to_string()
                                    }
                                }
                                "edit_file" => {
                                    if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                                        format!("edit: {}", path)
                                    } else {
                                        "edit_file".to_string()
                                    }
                                }
                                "web_search" => {
                                    if let Some(query) = input.get("query").and_then(|v| v.as_str())
                                    {
                                        let short_query = if query.len() > 40 {
                                            format!("{}...", &query[..40])
                                        } else {
                                            query.to_string()
                                        };
                                        format!("search: {}", short_query)
                                    } else {
                                        "web_search".to_string()
                                    }
                                }
                                "Skill" => {
                                    if let Some(skill) = input.get("skill").and_then(|v| v.as_str())
                                    {
                                        format!("skill: {}", skill)
                                    } else {
                                        "Skill".to_string()
                                    }
                                }
                                other => other.to_string(),
                            };

                            progress_guard.current_tool = Some(tool_display);
                        }

                        let output = execute_tool(config, todo_manager, skill_loader, name, input);

                        results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            is_error: None,
                            content: anthropic::types::ToolResultContent::Text(output),
                        });

                        {
                            let mut progress_guard = progress.lock().unwrap();
                            progress_guard.current_tool = None;
                        }
                    }
                }

                sub_messages.push(Message {
                    role: Role::Assistant,
                    content: response.content,
                });
                sub_messages.push(Message {
                    role: Role::User,
                    content: results,
                });
            }

            Some(StopReason::EndTurn) | Some(StopReason::StopSequence) | None => {
                // Normal end - extract text and return
                let mut text_result = None;
                for block in &response.content {
                    if let ContentBlock::Text { text } = block {
                        text_result = Some(text.clone());
                        break;
                    }
                }
                break text_result.unwrap_or_else(|| "(subagent returned no text)".to_string());
            }
        }
    };

    stop_signal.store(true, Ordering::Relaxed);
    updater.join().ok();

    let progress_guard = progress.lock().unwrap();
    let elapsed = progress_guard.start_time.elapsed();
    let tool_count = progress_guard.tool_count;
    drop(progress_guard);

    print!("\x1B[1A\x1B[K\x1B[1A\x1B[K");

    if result.starts_with("[ERROR]") {
        println!(
            "  {} {} - {} ({} tools, {:.1}s)",
            format!("[{}]", agent_type).bright_red(),
            description,
            "ERROR".bright_red(),
            tool_count,
            elapsed.as_secs_f64()
        );
    } else {
        println!(
            "  {} {} - {} ({} tools, {:.1}s)",
            format!("[{}]", agent_type).bright_magenta(),
            description,
            "done".bright_green(),
            tool_count,
            elapsed.as_secs_f64()
        );
    }

    result
}

fn execute_tool(
    config: &Config,
    todo_manager: &TodoManager,
    skill_loader: &SkillLoader,
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
        "web_search" => {
            // Note: web_search is async, so we return a placeholder
            // It will be handled in execute_tool_async
            "Error: web_search must be called via execute_tool_async".to_string()
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
        "Skill" => {
            let skill_name = input.get("skill").and_then(|v| v.as_str()).unwrap_or("");
            run_skill(skill_loader, skill_name)
        }
        _ => format!("Unknown tool: {}", name),
    }
}

async fn execute_tool_async(
    client: &Client,
    config: &Config,
    todo_manager: &TodoManager,
    skill_loader: &SkillLoader,
    name: &str,
    input: &serde_json::Value,
) -> String {
    if name == "Task" {
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("subtask");
        let prompt = input.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
        let agent_type = input
            .get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("explore");

        run_task(
            client,
            config,
            todo_manager,
            skill_loader,
            description,
            prompt,
            agent_type,
        )
        .await
    } else if name == "web_search" {
        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_i64())
            .unwrap_or(5) as usize;

        match web_search(query, max_results).await {
            Ok(results) => {
                if results.is_empty() {
                    format!("No search results found for: {}", query)
                } else {
                    let formatted: String = results
                        .iter()
                        .enumerate()
                        .map(|(i, r)| {
                            format!(
                                "{}. **{}**\n   URL: {}\n   {}\n",
                                i + 1,
                                r.title,
                                r.url,
                                r.snippet
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    format!("## Search Results for: {}\n\n{}", query, formatted)
                }
            }
            Err(e) => format!("Error performing web search: {}", e),
        }
    } else {
        execute_tool(config, todo_manager, skill_loader, name, input)
    }
}

// =============================================================================
// Token Management (from v3)
// =============================================================================

fn estimate_context_tokens(messages: &[Message], system: &str) -> usize {
    let messages_tokens: usize = messages
        .iter()
        .map(|msg| {
            msg.content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text.len() / 4, // Rough: 4 chars ≈ 1 token
                    ContentBlock::ToolUse { input, .. } => {
                        serde_json::to_string(input).unwrap_or_default().len() / 4
                    }
                    ContentBlock::ToolResult { content, .. } => match content {
                        anthropic::types::ToolResultContent::Text(t) => t.len() / 4,
                        _ => 100, // Default estimate
                    },
                    _ => 0,
                })
                .sum::<usize>()
        })
        .sum();

    let system_tokens = system.len() / 4;
    messages_tokens + system_tokens
}

fn calculate_max_tokens(messages: &[Message], system: &str, max_output_tokens: u32) -> u32 {
    const MAX_CONTEXT: usize = 200000;
    const OUTPUT_RATIO: f64 = 0.4;

    let context = estimate_context_tokens(messages, system);
    let available = MAX_CONTEXT.saturating_sub(context);
    let max_output = (available as f64 * OUTPUT_RATIO) as u32;

    max_output.min(max_output_tokens).max(4000)
}

// =============================================================================
// Main Agent Loop (adapted for v4 with Skills + Task + Todo)
// =============================================================================

async fn agent_loop(
    client: &Client,
    config: &Config,
    skill_loader: &SkillLoader,
    messages: &mut Vec<Message>,
) -> Result<()> {
    let todo_manager = TodoManager::new();

    let skill_descriptions = skill_loader.get_descriptions();
    let agent_descriptions = get_agent_descriptions();
    let system = config.system_prompt(&skill_descriptions, &agent_descriptions);

    let tools = create_all_tools(skill_loader);

    let mut consecutive_truncations = 0;

    loop {
        let max_tokens = calculate_max_tokens(messages, &system, config.max_output_tokens);

        let request = MessagesRequestBuilder::new(&config.model, messages.clone(), max_tokens)
            .system(SystemPrompt::Text(system.clone()))
            .tools(tools.clone())
            .build()?;

        let animation = spawn_thinking_animation();
        let response = client.messages(request).await?;
        drop(animation);

        match response.stop_reason {
            Some(StopReason::MaxTokens) => {
                consecutive_truncations += 1;

                println!(
                    "{} {}",
                    "Warning:".bright_yellow(),
                    format!(
                        "Response truncated (attempt {}/{})",
                        consecutive_truncations, config.max_truncation_retries
                    )
                    .bright_black()
                );

                if consecutive_truncations >= config.max_truncation_retries {
                    anyhow::bail!(
                        "Error: Response truncated {} times in a row. Task may be too complex.\n\n\
                         Hint: Break the task into smaller steps, or write large outputs\n\
                         to files using write_file.\n\n\
                         You can also increase MINI_CODE_MAX_OUTPUT_TOKENS (current: {})",
                        consecutive_truncations,
                        config.max_output_tokens
                    );
                }

                messages.push(Message {
                    role: Role::Assistant,
                    content: response.content,
                });

                messages.push(Message {
                    role: Role::User,
                    content: vec![ContentBlock::text(
                        "[SYSTEM: Your response was truncated due to length. \
                         Please provide a shorter summary of the key points \
                         (max 3-4 sentences), or write detailed content to a file instead.]",
                    )],
                });

                continue;
            }

            Some(StopReason::ToolUse) => {
                consecutive_truncations = 0;

                let mut tool_calls = Vec::new();
                for block in &response.content {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        tool_calls.push((id.clone(), name.clone(), input.clone()));
                    }
                }

                let mut results = Vec::new();
                for (id, name, input) in tool_calls {
                    // Display tool call
                    let tool_display = match name.as_str() {
                        "Task" => format!("{} {}", ">".bright_blue(), name.bright_magenta()),
                        "Skill" => format!("{} {}", ">".bright_blue(), name.bright_green()),
                        "TodoWrite" => format!("{} {}", ">".bright_blue(), name.bright_magenta()),
                        "bash" => format!("{} {}", ">".bright_blue(), name.bright_yellow()),
                        "web_search" => format!("{} {}", ">".bright_blue(), name.bright_cyan()),
                        _ => format!("{} {}", ">".bright_blue(), name.bright_cyan()),
                    };
                    println!("\n{}", tool_display);

                    let output = execute_tool_async(
                        client,
                        config,
                        &todo_manager,
                        skill_loader,
                        &name,
                        &input,
                    )
                    .await;

                    // Display output
                    let preview = if name == "TodoWrite"
                        || name == "Task"
                        || name == "Skill"
                        || name == "web_search"
                    {
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
                    } else if name == "Skill" {
                        println!(
                            "{} {}",
                            "Skill loaded:".bright_green(),
                            preview.lines().next().unwrap_or("").bright_black()
                        );
                    } else if name == "web_search" {
                        println!("{}", preview.bright_black());
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

            Some(StopReason::EndTurn) | Some(StopReason::StopSequence) | None => {
                // Normal end - display text and return
                for block in &response.content {
                    if let ContentBlock::Text { text } = block {
                        if !text.trim().is_empty() {
                            println!("{}", text);
                        }
                    }
                }

                messages.push(Message {
                    role: Role::Assistant,
                    content: response.content,
                });

                return Ok(());
            }
        }
    }
}

// =============================================================================
// Client Initialization (from v3)
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
// Input Handling (from v3)
// =============================================================================

#[cfg(feature = "readline")]
fn prompt_user() -> Result<String> {
    let mut rl = Editor::<(), DefaultHistory>::new()?;

    match rl.readline("You: ") {
        Ok(line) => {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                anyhow::bail!("Empty input")
            }
            Ok(trimmed.to_string())
        }
        Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
            println!("\nExiting...");
            std::process::exit(0);
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(not(feature = "readline"))]
fn prompt_user() -> Result<String> {
    print!("You: ");
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;

    let trimmed = line.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Empty input")
    }

    Ok(trimmed.to_string())
}

// =============================================================================
// Main Entry Point
// =============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    let client = create_client()?;
    let skill_loader = SkillLoader::new(&config.skills_dir);

    // Display startup info
    println!("{}", "=".repeat(60).bright_black());
    println!(
        "{} {} {}",
        "Mini Claude Code".bright_cyan().bold(),
        "v4".bright_magenta(),
        "(Skills + Subagents + Todo)".bright_black()
    );
    println!("{}", "=".repeat(60).bright_black());
    println!("{} {}", "Model:".bright_black(), config.model);
    println!("{} {}", "Workdir:".bright_black(), config.workdir.display());

    let skill_count = skill_loader.list_skills().len();
    if skill_count > 0 {
        println!("{} {} skills loaded", "Skills:".bright_green(), skill_count);
        for skill_name in skill_loader.list_skills() {
            println!("  {} {}", "-".bright_black(), skill_name.bright_green());
        }
    } else {
        println!(
            "{} {}",
            "Skills:".bright_black(),
            "none (create skills/ folder with SKILL.md files)".bright_yellow()
        );
    }

    println!("{}", "=".repeat(60).bright_black());
    println!();

    let mut messages = Vec::new();

    loop {
        let input = match prompt_user() {
            Ok(input) => input,
            Err(_) => continue,
        };

        messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::text(input)],
        });

        if let Err(e) = agent_loop(&client, &config, &skill_loader, &mut messages).await {
            eprintln!("{} {}", "Error:".bright_red(), e);
            messages.pop();
        }

        println!();
    }
}
