# v1_basic_agent - Mini Claude Code in Rust

This is a Rust implementation of the basic agent pattern from `learn-claude-code/v1_basic_agent.py`.

## Philosophy

The Model IS the Agent. This implementation demonstrates the core pattern used by Claude Code, Cursor Agent, and similar tools:

1. Give the model access to tools (bash, read_file, write_file, edit_file)
2. Run a loop that lets the model call tools until done
3. The model decides everything: which tools, in what order, when to stop

## The Four Essential Tools

| Tool       | Purpose              | Example                    |
|------------|----------------------|----------------------------|
| bash       | Run any command      | npm install, git status    |
| read_file  | Read file contents   | View src/index.ts          |
| write_file | Create/overwrite     | Create README.md           |
| edit_file  | Surgical changes     | Replace a function         |

## Setup

1. Create a `.env` file in the project root:

```env
ANTHROPIC_API_KEY=your_api_key_here
# Optional: use a custom API endpoint
ANTHROPIC_BASE_URL=https://your-custom-endpoint.com
# Optional: specify a different model
MODEL_NAME=claude-sonnet-4-20250514
```

2. Build and run:

```bash
cargo run -p v1_basic_agent
```

## Usage

Once running, you can interact with the agent:

```
You: Create a hello world Rust program in examples/hello.rs
```

The agent will:
1. Create the necessary directory structure
2. Write the Rust code
3. Report what it did

Type `exit`, `quit`, or `q` to exit.

## How It Works

The agent loop (in `main.rs`):

1. Sends messages + tools to Claude API
2. Receives response with tool calls
3. Executes each tool (safely, within workspace)
4. Feeds results back to the model
5. Repeats until model decides it's done

This simple pattern enables autonomous coding agents.

## Safety Features

- **Workspace isolation**: Files can only be accessed within the current directory
- **Dangerous command blocking**: Commands like `rm -rf /`, `sudo` are blocked
- **Timeouts**: Commands timeout after 60 seconds
- **Output limits**: Tool outputs truncated to 50KB to prevent context overflow

## Code Structure

```rust
// 1. Tool definitions (JSON schemas)
fn create_tools() -> Vec<Tool>

// 2. Tool implementations (actual functions)
fn run_bash(), run_read(), run_write(), run_edit()

// 3. The agent loop (the core pattern)
async fn agent_loop()

// 4. REPL (interactive interface)
async fn main()
```

## Differences from Python Version

- Uses `anthropic-rs` SDK instead of Python `anthropic` package
- Async/await instead of synchronous calls
- Colored output for better UX
- Same core logic and capabilities

## Next Steps

This basic agent can be extended with:
- More tools (git operations, LSP, etc.)
- Better error handling
- Streaming responses
- Multi-turn planning
- Permission systems
- Progress indicators

But this ~500 line implementation already demonstrates the core pattern that powers modern coding agents!
