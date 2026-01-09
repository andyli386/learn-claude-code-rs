# v2_todo_agent - Mini Claude Code with Structured Planning

This is a Rust implementation of the todo-based agent pattern from `learn-claude-code/v2_todo_agent.py`.

## Philosophy: "Make Plans Visible"

v1 works great for simple tasks. But ask it to "refactor auth, add tests, update docs" and watch what happens. Without explicit planning, the model:
- Jumps between tasks randomly
- Forgets completed steps
- Loses focus mid-way

## The Problem - "Context Fade"

In v1, plans exist only in the model's "head":

```
v1: "I'll do A, then B, then C"  (invisible)
    After 10 tool calls: "Wait, what was I doing?"
```

## The Solution - TodoWrite Tool

v2 adds ONE new tool that fundamentally changes how the agent works:

```
v2:
  [ ] Refactor auth module
  [>] Add unit tests         <- Currently working on this
  [ ] Update documentation
```

Now both YOU and the MODEL can see the plan. The model can:
- Update status as it works
- See what's done and what's next
- Stay focused on one task at a time

## Key Constraints (Guardrails)

| Rule              | Why                              |
|-------------------|----------------------------------|
| Max 20 items      | Prevents infinite task lists     |
| One in_progress   | Forces focus on one thing        |
| Required fields   | Ensures structured output        |

## The Deep Insight

> **"Structure constrains AND enables."**

Todo constraints (max items, one in_progress) ENABLE visible plan and tracked progress.

This pattern appears everywhere in agent design:
- `max_tokens` constrains → enables manageable responses
- Tool schemas constrain → enable structured calls
- Todos constrain → enable complex task completion

Good constraints aren't limitations. They're scaffolding.

## Setup

1. Create a `.env` file in the project root:

```env
# API Key (use either one)
ANTHROPIC_API_KEY=your_api_key_here
# OR
ANTHROPIC_AUTH_TOKEN=your_api_key_here

# Base URL (use either one, optional)
ANTHROPIC_API_BASE=https://your-custom-endpoint.com
# OR
ANTHROPIC_BASE_URL=https://your-custom-endpoint.com

# Optional settings
ANTHROPIC_API_VERSION=2023-06-01
MODEL_NAME=claude-sonnet-4-20250514
```

The client supports both standard and alternative environment variable names for better compatibility.

2. Build and run:

```bash
cargo run -p v2_todo_agent
```

## Usage

Once running, the agent will prompt you to use TodoWrite for multi-step tasks:

```
You: Refactor the authentication module, add tests, and update docs

> TodoWrite
  [ ] Refactor authentication module
  [>] Add unit tests <- Adding unit tests for auth module
  [ ] Update documentation

  (0/3 completed)
```

The agent automatically:
- Creates a todo list when you give it multiple tasks
- Marks tasks as `in_progress` before starting
- Marks tasks as `completed` when done
- Shows you progress in real-time

## Features

### 5 Essential Tools

1. **bash** - Run any command
2. **read_file** - Read file contents
3. **write_file** - Create/overwrite files
4. **edit_file** - Surgical edits
5. **TodoWrite** - Track and plan tasks (NEW in v2)

### Todo Item Structure

Each todo item has:
- `content`: Task description
- `status`: `pending` | `in_progress` | `completed`
- `activeForm`: Present tense action (e.g., "Adding tests...")

### Reminder System

The agent uses soft reminders to encourage todo usage:
- **Initial reminder**: Shown at the start of conversation
- **Nag reminder**: Shown if 10+ turns pass without todo update

These are gentle hints, not hard requirements.

## How It Works

```rust
// TodoManager validates and renders the todo list
struct TodoManager {
    items: Arc<Mutex<Vec<TodoItem>>>,
}

// Each todo item has required fields
struct TodoItem {
    content: String,
    status: TodoStatus,  // pending | in_progress | completed
    active_form: String, // "Adding tests..."
}

// Validation enforces constraints
fn update(&self, items: Vec<TodoItem>) -> Result<String> {
    // Max 20 items
    // Only one in_progress at a time
    // All required fields present
}
```

## Example Session

```
You: Create a new Rust module for user management with CRUD operations

> TodoWrite
  [>] Creating user management module <- Creating module structure
  [ ] Implement Create operation
  [ ] Implement Read operation
  [ ] Implement Update operation
  [ ] Implement Delete operation
  [ ] Add tests

  (0/6 completed)

> write_file
  Wrote 523 bytes to src/users.rs

> TodoWrite
  [x] Creating user management module
  [>] Implement Create operation <- Implementing Create function
  [ ] Implement Read operation
  [ ] Implement Update operation
  [ ] Implement Delete operation
  [ ] Add tests

  (1/6 completed)

...
```

## Differences from Python Version

- Uses `anthropic-rs` SDK instead of Python `anthropic` package
- Async/await instead of synchronous calls
- Thread-safe TodoManager using `Arc<Mutex>`
- Same core logic and capabilities
- Colored output for better UX
- Optional readline support for better input handling

## Code Structure

```rust
// 1. TodoManager - Validates and renders todo list
struct TodoManager { ... }

// 2. Tool definitions (v1 tools + TodoWrite)
fn create_tools() -> Vec<Tool> { ... }

// 3. Tool implementations
fn execute_tool(...) -> String { ... }

// 4. Agent loop with todo tracking
async fn agent_loop(...) { ... }

// 5. REPL with reminder injection
async fn main() { ... }
```

## Safety Features

Same as v1:
- Workspace isolation
- Dangerous command blocking
- Command timeouts (60s)
- Output limits (50KB)

## Next Steps

This implementation demonstrates:
- How structured planning improves complex task completion
- The power of constraints in agent design
- Real-time progress visibility
- Focused, single-task execution

Future enhancements could include:
- Persistent todo lists across sessions
- Subtasks and hierarchical planning
- Time estimates and deadlines
- Todo templates for common workflows
