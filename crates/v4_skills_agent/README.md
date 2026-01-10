# v4_skills_agent - Skills Mechanism (~1700 lines)

**Core Philosophy**: Knowledge Externalization

## What's New in v4?

v3 gave us subagents for task decomposition. But there's a deeper question:

> How does the model know **HOW** to handle domain-specific tasks?

- Processing PDFs? It needs to know pdftotext vs PyMuPDF
- Building MCP servers? It needs protocol specs and best practices
- Code review? It needs a systematic checklist

This knowledge isn't a tool - it's **EXPERTISE**. Skills solve this by letting the model load domain knowledge on-demand.

## The Paradigm Shift: Knowledge Externalization

| Traditional AI | Skills |
|----------------|--------|
| Knowledge locked in model parameters | Knowledge stored in editable files |
| To teach new skills: collect data → train → deploy | To teach new skills: write a SKILL.md file |
| Cost: $10K-$1M+, Timeline: Weeks | Cost: Free, Timeline: Minutes |
| Requires ML expertise, GPU clusters | Anyone can do it |

**It's like attaching a hot-swappable LoRA adapter without any training!**

## Tools vs Skills

| Concept | What it is | Example |
|---------|-----------|---------|
| **Tool** | What model CAN do | bash, read_file, write |
| **Skill** | How model KNOWS to do | PDF processing, MCP dev |

**Tools are capabilities. Skills are knowledge.**

## Progressive Disclosure

```
Layer 1: Metadata (always loaded)      ~100 tokens/skill
         name + description only

Layer 2: SKILL.md body (on trigger)    ~2000 tokens
         Detailed instructions

Layer 3: Resources (as needed)         Unlimited
         scripts/, references/, assets/
```

This keeps context lean while allowing arbitrary depth.

## SKILL.md Standard

```
skills/
├── pdf/
│   ├── SKILL.md          # Required: YAML frontmatter + Markdown body
│   ├── scripts/          # Optional: helper scripts
│   └── references/       # Optional: docs, specs
├── mcp-builder/
│   └── SKILL.md
└── code-review/
    └── SKILL.md
```

### SKILL.md Format

```markdown
---
name: pdf
description: Process PDF files. Use when reading, creating, or merging PDFs.
---

# PDF Processing Skill

## Reading PDFs

Use pdftotext for quick extraction:
```bash
pdftotext input.pdf -
```

For complex PDFs with tables, use PyMuPDF:
```python
import fitz  # PyMuPDF
doc = fitz.open("input.pdf")
for page in doc:
    print(page.get_text())
```

## Merging PDFs

Use PyPDF2:
```python
from PyPDF2 import PdfMerger
merger = PdfMerger()
merger.append("file1.pdf")
merger.append("file2.pdf")
merger.write("merged.pdf")
```
```

## Cache-Preserving Injection

**Critical insight**: Skill content goes into `tool_result` (user message), NOT system prompt. This preserves prompt cache!

```
Wrong: Edit system prompt each time (cache invalidated, 20-50x cost)
Right: Append skill as tool result (prefix unchanged, cache hit)
```

This is how production Claude Code works - and why it's cost-efficient.

## Architecture

```
v4_skills_agent = v2_todo_agent + v3_subagent + Skills

Components:
- SkillLoader: Parse SKILL.md files with YAML frontmatter
- Skill tool: Load skills on-demand (available to main agent AND subagents)
- Task tool: Spawn subagents (from v3)
- TodoWrite: Track progress (from v2)
- Base tools: bash, read_file, write_file, edit_file

Subagent Skill Support:
- explore: ✅ Can use skills for analysis patterns
- code: ✅ Can use skills for implementation guidance
- plan: ✅ Can use skills for design patterns
```

## Subagent Skills Support (NEW!)

**Key Feature**: Subagents can now load skills just like the main agent!

### Why This Matters

**Before**:
- ❌ Subagents couldn't access domain knowledge
- ❌ Had to rely on parent agent for expertise
- ❌ Limited effectiveness for domain-specific tasks

**After**:
- ✅ Subagents can load skills on-demand
- ✅ Shared knowledge base, isolated context
- ✅ More intelligent task decomposition

### Example: PDF Analysis with Subagent Skills

```
User: Analyze the PDF processing code using a subagent

Main Agent:
  > Task(explore): "Analyze PDF implementation"
    [explore] ... 4 tools, 3.2s

    Subagent (explore):
      > Skill pdf
        Loaded: PDF processing best practices

      > read_file src/pdf_handler.rs
        Analyzing...

      Returns: "Code uses PyMuPDF, follows best practices for table extraction..."

  Done! The subagent used the PDF skill to provide expert analysis.
```

### Smart Task Decomposition

```
User: Implement PDF export feature

Main Agent:
  1. Task(explore) + Skill(code-review)
     → Analyze existing code structure

  2. Task(plan) + Skill(pdf) + Skill(architecture)
     → Design PDF export approach

  3. Task(code) + Skill(pdf)
     → Implement with best practices
```

Each subagent gets domain knowledge exactly when needed!

## Usage

```bash
# Run the agent
cargo run -p v4_skills_agent

# Or with release build
cargo run --release -p v4_skills_agent
```

### Creating Skills

1. **Create skills directory**:
```bash
mkdir -p skills/my-skill
```

2. **Create SKILL.md**:
```bash
cat > skills/my-skill/SKILL.md << 'EOF'
---
name: my-skill
description: Brief description of what this skill does
---

# My Skill

## Overview

Detailed instructions on how to use this skill...

## Examples

```bash
# Example commands
echo "Hello from skill!"
```

## Best Practices

- Tip 1
- Tip 2
EOF
```

3. **Optional resources**:
```bash
mkdir -p skills/my-skill/scripts
mkdir -p skills/my-skill/references
mkdir -p skills/my-skill/assets
```

## Example Session

```
You: Help me extract text from a PDF file

> Skill pdf
Skill loaded: # Skill: pdf

> bash pdftotext document.pdf -
[PDF text content...]

Extracted the text successfully. The PDF contains...
```

## Configuration

Same as v2 and v3:

```bash
# .env file
ANTHROPIC_API_KEY=your-key-here
MODEL_NAME=claude-sonnet-4-20250514
MINI_CODE_MAX_OUTPUT_TOKENS=160000
MINI_CODE_MAX_TRUNCATION_RETRIES=3
```

## Implementation Highlights

### SkillLoader (lines 228-405)

- **parse_skill_md()**: Uses regex to extract YAML frontmatter and markdown body
  ```rust
  let re = Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n(.*)$")?;
  ```

- **load_skills()**: Scans skills/ directory for SKILL.md files

- **get_descriptions()**: Returns Layer 1 metadata for system prompt

- **get_skill_content()**: Returns Layer 2 full content + Layer 3 resource hints

### Skill Tool (lines 703-732)

Dynamically generated description listing all available skills:

```rust
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
..."#,
            skill_loader.get_descriptions()
        ),
        ...
    }
}
```

### run_skill() (lines 914-949)

Cache-preserving skill injection:

```rust
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
        None => format!("Error: Unknown skill '{}'...", skill_name)
    }
}
```

Returns content wrapped in `<skill-loaded>` tags, injected as tool_result (user message).

## Key Features

✅ **Knowledge Externalization**: Store expertise in files, not model weights
✅ **Progressive Disclosure**: Load only what's needed, when it's needed
✅ **Cache-Preserving**: Skills injected via tool_result to preserve prompt cache
✅ **Extensible**: Anyone can create skills without code changes
✅ **Composable**: Skills + Subagents + Todo tracking work together
✅ **Zero Training Cost**: Add new skills in minutes for free

## Comparison

| Feature | v2_todo_agent | v3_subagent | v4_skills_agent |
|---------|---------------|-------------|-----------------|
| Base tools | ✅ | ✅ | ✅ |
| Todo tracking | ✅ | ✅ | ✅ |
| Subagents | ❌ | ✅ | ✅ |
| Skills | ❌ | ❌ | ✅ |
| Token mgmt | ✅ | ✅ | ✅ |
| Progress UI | ❌ | ✅ | ✅ |
| Lines of code | ~850 | ~1400 | ~1700 |

## Philosophy

The progression from v0 → v4 demonstrates increasing **separation of concerns**:

- **v0**: Pure tool execution
- **v1**: + Conversation state
- **v2**: + Task tracking
- **v3**: + Task decomposition (subagents)
- **v4**: + Knowledge externalization (skills)

Each layer builds on the previous, creating a more capable system while maintaining simplicity.

## Related Docs

- v2_todo_agent: TodoManager implementation
- v3_subagent: Subagent mechanism
- ENV_CONFIG_QUICK.md: Environment variable reference
- CRASH_FIX_SUMMARY.md: Token management details

## License

MIT
