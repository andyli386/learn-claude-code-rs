# Model Selection Feature - Quick Reference

## 🚀 Quick Start

### 1. Update your `.env` file

Run the setup script:
```bash
cd /home/vincent/project/learn-claude-code-rs
./crates/v0_bash_agent/setup_env.sh
```

Or manually update `.env`:
```env
ANTHROPIC_AUTH_TOKEN=sk-m68t4Fiq4clAyA4016PyhPORaK1p3icmPmf1CbWrqmAsYs8l
ANTHROPIC_BASE_URL=https://xz.ai2api.dev/
MODEL_NAME=claude-sonnet-4-5-20250929
```

### 2. Run v0_bash_agent

## 📖 Usage Patterns

### Interactive Mode

```bash
# Default model (Sonnet - from .env or fallback)
cargo run -p v0_bash_agent

# Explicitly use Sonnet
cargo run -p v0_bash_agent sonnet

# Use Opus for complex tasks
cargo run -p v0_bash_agent opus
```

### One-Shot Task Mode

```bash
# Default model
cargo run -p v0_bash_agent "your task here"

# With Sonnet (faster)
cargo run -p v0_bash_agent sonnet "analyze codebase structure"

# With Opus (more capable)
cargo run -p v0_bash_agent opus "deep code review with suggestions"
```

## 🤖 Available Models

| Alias | Full Model Name | Best For |
|-------|----------------|----------|
| `sonnet` | `claude-sonnet-4-5-20250929` | General tasks, faster responses |
| `opus` | `claude-opus-4-5-20251101` | Complex analysis, code reviews |

## 💡 Examples

### Example 1: Interactive with Sonnet
```bash
cargo run -p v0_bash_agent sonnet
```
Output:
```
🤖 Using model: Claude 4.5 Sonnet (faster) (claude-sonnet-4-5-20250929)
Type 'q' or 'exit' to quit. Type 'help' for usage examples.

>> list all rust files
$ find . -name "*.rs"
...
```

### Example 2: One-shot with Opus
```bash
cargo run -p v0_bash_agent opus "analyze the architecture of this project"
```

### Example 3: Spawn Subagent with Different Model
```bash
cargo run -p v0_bash_agent
>> v0_bash_agent opus "detailed code quality analysis"
```

## 🔧 Argument Parsing Logic

1. **No arguments** → Interactive mode with default model
2. **First arg is `sonnet` or `opus`**:
   - No second arg → Interactive mode with specified model
   - With second arg → One-shot mode with specified model
3. **First arg is not a model alias** → One-shot mode with default model

## 📝 Notes

- Model selection works in both interactive and one-shot modes
- You can spawn subagents with different models from within the agent
- The `help` command in interactive mode shows usage examples
- Default model can be set in `.env` via `MODEL_NAME`
- Model aliases are case-insensitive (`sonnet`, `Sonnet`, `SONNET` all work)

## 🛠️ Utility Scripts

- `setup_env.sh` - Update .env with correct model
- `demo_model_selection.sh` - Demo all model selection modes
- `quick_test.sh` - Quick test with current env
- `test_models` binary - Test which models work with your API
