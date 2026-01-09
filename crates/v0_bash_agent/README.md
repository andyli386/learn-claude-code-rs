# v0_bash_agent - Mini Claude Code in Rust

A Rust implementation of the minimal coding agent based on the Python version from [learn-claude-code](https://github.com/anthropics/learn-claude-code).

## Core Philosophy: "Bash is All You Need"

This is the ULTIMATE simplification of a coding agent. The essence of an agent is:

**ONE tool (bash) + ONE loop = FULL agent capability**

## Features

- **Single Tool**: Only bash command execution
- **Recursive Subagents**: Agents can spawn other agents via bash
- **Interactive REPL**: Chat mode for interactive problem-solving
- **Subagent Mode**: Can be called by parent agents

## Installation

1. Copy `.env.example` to `.env` and configure your API credentials:

```bash
cp .env.example .env
```

2. Edit `.env` with your settings. The following environment variables are supported:

**Standard naming (recommended):**
```env
ANTHROPIC_API_KEY=sk-ant-xxxxx
ANTHROPIC_API_BASE=https://api.anthropic.com
MODEL_NAME=claude-sonnet-4-5-20250929
```

**Alternative naming (also supported):**
```env
ANTHROPIC_AUTH_TOKEN=sk-ant-xxxxx  # Alternative to ANTHROPIC_API_KEY
ANTHROPIC_BASE_URL=https://xxxxx   # Alternative to ANTHROPIC_API_BASE
MODEL_NAME=claude-sonnet-4-5-20250929
```

**Available Models** (tested with API proxies):
- `claude-sonnet-4-5-20250929` - Claude 4.5 Sonnet (faster, recommended)
- `claude-opus-4-5-20251101` - Claude 4.5 Opus (more capable)

The code automatically detects and supports both naming conventions, so you can use whichever your setup requires.

3. Build the project:

```bash
cargo build --release -p v0_bash_agent
```

## Usage

### Interactive Mode (REPL)

**Default (uses Sonnet automatically):**
```bash
cargo run -p v0_bash_agent
```

**Use Opus for complex tasks:**
```bash
cargo run -p v0_bash_agent opus
```

Or run the compiled binary:

```bash
./target/release/v0_bash_agent          # Uses Sonnet by default
./target/release/v0_bash_agent opus     # Uses Opus
```

Then interact with the agent:

```
🤖 Using model: Claude 4.5 Sonnet (faster) (claude-sonnet-4-5-20250929)
Type 'q' or 'exit' to quit. Type 'help' for usage examples.

>> list all rust files in this project
$ find . -name "*.rs" -type f
./crates/v0_bash_agent/src/main.rs
...

>> help
Usage Examples:
  Basic commands:
    >> ls -la
    >> cat README.md
...
```

### Subagent Mode (One-Shot)

Execute a single task and exit:

**Default (uses Sonnet automatically):**
```bash
cargo run -p v0_bash_agent "explore the codebase and summarize the structure"
```

**Use Opus for complex analysis:**
```bash
cargo run -p v0_bash_agent opus "analyze code quality and suggest improvements"
```

Or with the compiled binary:

```bash
./target/release/v0_bash_agent "list all TODO comments in rust files"
./target/release/v0_bash_agent opus "deep code review"
```

### Spawning Subagents

Agents can spawn other agents for complex tasks:

```
>> analyze the project architecture
$ v0_bash_agent "find all rust files and summarize their purpose"
[Subagent output...]

>> get detailed code review with opus
$ v0_bash_agent opus "review code quality and performance"
[Subagent using Opus model...]
```

**Model Selection Summary:**
- No argument: Uses default model from `.env` (or Sonnet if not set)
- `sonnet`: Uses Claude 4.5 Sonnet (faster, recommended for most tasks)
- `opus`: Uses Claude 4.5 Opus (more capable, for complex analysis)
- Custom: Set `MODEL_NAME` in `.env` for other models

## How It Works

1. **Single Tool Definition**: Only bash command execution
2. **Agentic Loop**:
   - Call Claude with messages and tools
   - Execute any tool calls (bash commands)
   - Append results and repeat
   - Stop when no more tool calls
3. **Process Isolation**: Subagents run as separate processes with their own context

## Architecture

```
Main Agent (REPL)
  |
  |-- bash: cargo build
  |-- bash: v0_bash_agent "analyze errors"  ← Spawns subagent
       |
       |-- Subagent (isolated process)
            |-- bash: grep "error" output.log
            |-- bash: cat src/main.rs
            |-- Returns: "Found 3 type errors..."
```

## Why Bash is Enough

Everything you need can be done through bash:

| Need | Bash Command |
|------|--------------|
| Read files | `cat`, `head`, `tail`, `grep` |
| Write files | `echo '...' > file` |
| Search | `find`, `grep`, `rg`, `ls` |
| Execute | `cargo`, `npm`, any command |
| Subagent | `v0_bash_agent "task"` |

## Testing

The project includes comprehensive unit and integration tests.

### Run All Tests

```bash
cargo test -p v0_bash_agent
```

### Run Only Unit Tests

```bash
cargo test -p v0_bash_agent --lib
```

### Run Only Integration Tests

```bash
cargo test -p v0_bash_agent --test integration_test
```

### Test Coverage

**Unit Tests** (in `src/lib.rs`):
- ✅ `test_get_cwd` - Current working directory retrieval
- ✅ `test_get_bash_tool` - Bash tool definition
- ✅ `test_get_system_prompt` - System prompt generation
- ✅ `test_execute_bash_simple_command` - Simple command execution
- ✅ `test_execute_bash_with_newline` - Multi-line output handling
- ✅ `test_execute_bash_pwd` - Path commands
- ✅ `test_execute_bash_error_command` - Error handling
- ✅ `test_execute_bash_with_pipe` - Piped commands
- ✅ `test_execute_bash_ls` - Directory listing

**Integration Tests** (in `tests/integration_test.rs`):
- ✅ `test_integration_bash_commands` - Command chaining
- ✅ `test_bash_tool_structure` - Tool structure validation
- ✅ `test_system_prompt_contains_required_info` - Prompt completeness
- ✅ `test_cwd_is_valid` - Working directory validity
- ✅ `test_execute_bash_file_operations` - File I/O operations
- ✅ `test_execute_bash_with_environment_variables` - Environment variables
- ✅ `test_execute_bash_multiline_script` - Multi-line scripts
- ✅ `test_execute_bash_grep_pattern` - Pattern matching
- ✅ `test_execute_bash_find_files` - File discovery
- ✅ `test_execute_bash_error_handling` - Error scenarios

All tests pass! ✅

## Comparison with Python Version

This Rust implementation follows the same philosophy as the Python version:
- ~200 lines of core logic
- Single tool (bash)
- Interactive and subagent modes
- Process-based agent isolation

Key differences:
- Written in Rust using the `anthropic-rs` SDK
- Async/await with Tokio
- Type-safe message handling
- Compiled binary for faster startup
- **Comprehensive test suite with 19 tests**

## License

MIT
