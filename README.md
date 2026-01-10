# Mini Claude Code - 编码代理的渐进式实现

一个通过三个版本逐步演进的项目，展示如何构建一个功能完整的 AI 编码代理。每个版本都建立在前一个版本的基础上，逐步添加功能和复杂性。

## 项目概览

这是一个 Rust workspace 项目，包含三个独立的 crate，代表编码代理的三个演进阶段：

- **v0_bash_agent**: 极简代理 - 仅使用 bash 工具
- **v1_basic_agent**: 基础代理 - 添加 4 个核心工具
- **v2_todo_agent**: 高级代理 - 增加任务规划和可见性

每个版本都是完全可用的，展示了从简单到复杂的设计演进。

## 核心设计理念

### "模型即代理"

这个项目揭示了一个关键洞察：编码代理的核心是一个循环，让模型重复调用工具直到任务完成。

```
传统助手:
  用户 → 模型 → 文本响应

代理系统:
  用户 → 模型 → [工具调用 → 结果]* → 最终响应
                      ↑________|
```

星号 (*) 很重要！模型会**反复调用工具**，直到它认为任务完成。这将聊天机器人转变为自主代理。

### 为什么选择 Rust？

- **类型安全**: 编译时捕获错误
- **性能**: 零成本抽象，高效执行
- **并发**: Tokio 异步运行时，轻松处理多任务
- **可靠性**: 内存安全，适合长时间运行的代理

## 版本对比

| 特性 | v0_bash_agent | v1_basic_agent | v2_todo_agent |
|------|--------------|----------------|---------------|
| 工具数量 | 1 (bash) | 4 (bash + 文件操作) | 5 (增加 TodoWrite) |
| 代码行数 | ~200 行 | ~500 行 | ~800 行 |
| 子代理支持 | ✅ 进程级隔离 | ❌ | ❌ |
| 任务规划 | ❌ | ❌ | ✅ 可见任务列表 |
| 适用场景 | 快速原型、单步任务 | 日常编码 | 复杂重构、多步骤任务 |

## 快速开始

### 环境要求

- Rust 1.70+ (建议使用 stable)
- API Key for Anthropic Claude

### 安装

```bash
# 克隆项目
git clone <repo-url>
cd mini-code

# 复制环境变量配置
cp .env.example .env

# 编辑 .env 文件，添加你的 API Key
# ANTHROPIC_API_KEY=your_key_here
```

### 运行

#### v0 - 极简代理 (推荐用于快速任务)

```bash
# 交互模式
cargo run --bin v0_bash_agent

# 子代理模式（一次性执行任务）
cargo run --bin v0_bash_agent -- "分析 src/ 目录结构"

# 使用不同的模型
cargo run --bin v0_bash_agent -- opus "审查代码质量"
```

#### v1 - 基础代理（日常编码）

```bash
cargo run --bin v1_basic_agent
```

#### v2 - 高级代理（复杂任务）

```bash
# 默认启用 readline 支持（更好的 UTF-8 输入）
cargo run --bin v2_todo_agent

# 不使用 readline
cargo run --bin v2_todo_agent --no-default-features
```

## 版本详解

### v0: Bash 是万能的

**核心哲学**: Unix 哲学说一切皆文件，一切可管道。Bash 是通往这个世界的门户。

**设计洞察**:
- 单一工具 (bash) + 单一循环 = 完整的代理能力
- 通过 bash 调用自身实现子代理（进程隔离 = 上下文隔离）
- 代码简洁 (~200 行)，易于理解和修改

**工具映射**:

| 你需要 | Bash 命令 |
|--------|-----------|
| 读文件 | cat, head, tail, grep |
| 写文件 | echo '...' > file, cat << 'EOF' > file |
| 搜索 | find, grep, rg, ls |
| 执行 | cargo, npm, make, 任何命令 |
| **子代理** | v0_bash_agent "task" |

**何时使用**:
- 快速原型开发
- 简单的文件操作
- 需要隔离的子任务
- 学习代理的基本原理

### v1: 模型即代理

**核心哲学**: 代码只是提供工具和运行循环，模型才是决策者。

**新增内容**:
- 4 个核心工具：bash, read_file, write_file, edit_file
- 工具结果截断（防止上下文溢出）
- 路径安全检查（防止访问工作区外的文件）
- UTF-8 安全截断
- 思考动画（改善用户体验）
- 10 分钟超时保护
- 友好的错误提示

**工具设计**:

| 工具 | 用途 | 示例 |
|------|------|------|
| bash | 运行任何命令 | npm install, git status |
| read_file | 读取文件内容 | 查看 src/main.rs |
| write_file | 创建/覆盖文件 | 创建 README.md |
| edit_file | 精确修改 | 替换一个函数 |

**何时使用**:
- 日常编码任务
- 需要精确的文件编辑
- 需要更好的安全性
- 多步骤的简单任务

### v2: 可见规划

**核心哲学**: "让计划可见" - 结构既约束又赋能。

**新增内容**:
- TodoWrite 工具：创建可见的任务列表
- 任务约束（最多 20 项，最多 1 个进行中）
- 强制字段（content, status, activeForm）
- 实时进度跟踪
- 更智能的系统提示（包含工作流指导）
- Readline 支持（可选，改善 UTF-8 输入）
- 任务提醒（超过 10 次工具调用未更新）

**TodoWrite 工具示例**:

```json
{
  "items": [
    {
      "content": "读取和分析代码库结构",
      "status": "in_progress",
      "activeForm": "正在分析代码库结构"
    },
    {
      "content": "识别关键组件和模式",
      "status": "pending",
      "activeForm": "识别组件中"
    },
    {
      "content": "编写分析报告",
      "status": "pending",
      "activeForm": "编写报告中"
    }
  ]
}
```

**显示效果**:

```
[x] 已完成的任务
[>] 进行中的任务 <- 正在做这个...
[ ] 待处理任务

(1/3 completed)
```

**约束的价值**:
- 最多 20 项 → 防止无限任务列表
- 只能 1 个进行中 → 强制专注
- 必需字段 → 确保结构化输出

**何时使用**:
- 复杂的多步骤任务
- 需要明确规划的重构
- 想要看到代理的思考过程
- 长时间运行的任务

## 架构设计

### 工作空间结构

```
mini-code/
├── Cargo.toml                 # Workspace 配置
├── crates/
│   ├── v0_bash_agent/        # 极简版本
│   │   ├── src/
│   │   │   ├── main.rs       # 二进制入口
│   │   │   ├── lib.rs        # 库代码（可测试）
│   │   │   └── bin/          # 辅助工具
│   │   └── Cargo.toml
│   ├── v1_basic_agent/       # 基础版本
│   │   ├── src/
│   │   │   └── main.rs       # 完整实现 + 测试
│   │   └── Cargo.toml
│   └── v2_todo_agent/        # 高级版本
│       ├── src/
│       │   └── main.rs       # 完整实现 + 测试
│       └── Cargo.toml
├── .env.example              # 环境变量模板
└── README.md
```

### 共享依赖

所有版本都使用 workspace 共享依赖：

```toml
[workspace.dependencies]
anthropic = { git = "https://github.com/andyli386/anthropic-rs.git" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dotenvy = "0.15"
anyhow = "1"
colored = "3"
```

### 核心循环（所有版本共享）

```rust
while not_done {
    // 1. 调用模型
    response = model(messages, tools);

    // 2. 检查是否需要工具调用
    if no_tool_calls(response) {
        return response;  // 任务完成
    }

    // 3. 执行工具
    results = execute_tools(response.tool_calls);

    // 4. 将结果添加到对话历史
    messages.append(assistant_message);
    messages.append(user_message_with_results);
}
```

## 环境变量

创建 `.env` 文件（参考 `.env.example`）：

```bash
# 必需：API Key
ANTHROPIC_API_KEY=your_api_key_here

# 可选：自定义 API Base URL
ANTHROPIC_API_BASE=https://api.anthropic.com

# 可选：模型选择
MODEL_NAME=claude-sonnet-4-5-20250929
```

支持的模型：
- `claude-sonnet-4-5-20250929` (更快，推荐)
- `claude-opus-4-5-20251101` (更强大)

## 开发工具

项目配置了多个开发工具：

### Cargo 工具

```bash
# 依赖安全检查
cargo deny check

# 拼写检查
typos

# 生成 changelog
git cliff

# 增强测试
cargo nextest run
```

### Pre-commit 钩子

```bash
# 安装
pipx install pre-commit
pre-commit install

# 手动运行
pre-commit run --all-files
```

## 测试

每个版本都包含单元测试：

```bash
# 运行所有测试
cargo test

# 运行特定版本的测试
cargo test -p v0_bash_agent
cargo test -p v1_basic_agent
cargo test -p v2_todo_agent

# 使用 nextest（更快）
cargo nextest run
```

### 测试覆盖

- **v0**: bash 命令执行、系统提示生成
- **v1**: UTF-8 截断、路径安全、工具创建
- **v2**: TodoManager 验证、约束执行、完整工作流

## 使用示例

### 示例 1: 快速文件分析 (v0)

```bash
cargo run --bin v0_bash_agent -- "找出所有 .rs 文件并统计行数"
```

代理会执行：
```bash
find . -name "*.rs"
wc -l $(find . -name "*.rs")
```

### 示例 2: 代码重构 (v1)

```
你: 将 config.rs 中的 Config 结构体重命名为 Configuration

代理:
> read_file config.rs
> edit_file config.rs "struct Config" "struct Configuration"
> edit_file config.rs "impl Config" "impl Configuration"
> grep -r "Config::" src/
> (报告完成)
```

### 示例 3: 复杂重构 (v2)

```
你: 重构认证模块，添加测试，更新文档

代理:
TodoWrite: [
  [x] 分析当前认证实现
  [>] 编写单元测试
  [ ] 更新文档
]

> read_file src/auth.rs
> write_file src/auth_test.rs ...
> (继续执行)
```

## 性能考虑

- **超时**: 所有版本都有 10 分钟超时保护
- **输出截断**: 工具输出限制在 50KB 以防止上下文溢出
- **UTF-8 安全**: 字符边界对齐的截断，防止无效 UTF-8
- **路径验证**: 防止访问工作区外的文件

## 安全特性

- **路径验证**: 所有文件操作限制在工作区内
- **危险命令拦截**: 阻止 `rm -rf /` 等危险操作
- **进程隔离**: v0 的子代理在独立进程中运行
- **只读默认**: 文件操作需要明确的工具调用

## 故障排查

### API 错误

```
API Error: insufficient balance
Hint: Your API account balance is insufficient. Please recharge.
```

**解决方案**: 检查 API 账户余额

### 超时错误

```
API Error: Request timed out after 10 minutes
Hint: Request timed out. The task may be too complex or the API server is slow.
```

**解决方案**: 将任务分解为更小的子任务

### UTF-8 输入问题

如果遇到 UTF-8 字符输入问题，使用 v2 的 readline 特性：

```bash
cargo run --bin v2_todo_agent --features readline
```

## 学习路径

建议按顺序阅读和运行每个版本：

1. **先运行 v0**: 理解代理的基本循环
2. **阅读 v0 代码**: 注意 `chat()` 函数中的循环
3. **运行 v1**: 体验更多工具的便利
4. **比较 v0 和 v1**: 看看 4 个工具如何改变体验
5. **运行 v2**: 尝试复杂任务
6. **阅读 v2 代码**: 理解 TodoWrite 如何实现可见规划

## 贡献

欢迎贡献！特别是：

- 添加新的工具实现
- 改进错误处理
- 增加测试覆盖
- 优化性能
- 改进文档

## 许可证

MIT License

## 致谢

本项目受到以下项目的启发：
- Anthropic Claude Code
- Cursor AI
- OpenAI Codex

## 相关文档

- [TIMEOUT_IMPROVEMENTS.md](./TIMEOUT_IMPROVEMENTS.md) - 超时处理改进
- [UTF8_TRUNCATION_FIX.md](./UTF8_TRUNCATION_FIX.md) - UTF-8 截断修复
- [ERROR_HANDLING_IMPROVEMENTS.md](./ERROR_HANDLING_IMPROVEMENTS.md) - 错误处理改进
- [UNIT_TESTS.md](./UNIT_TESTS.md) - 单元测试说明
- [UI_IMPROVEMENTS.md](./UI_IMPROVEMENTS.md) - UI 改进

## 常见问题

### Q: 为什么有三个版本？

A: 每个版本展示了不同的设计权衡。v0 最简单，v1 平衡功能与复杂性，v2 最强大。你可以根据需求选择合适的版本。

### Q: 哪个版本最适合生产使用？

A: 取决于你的需求：
- 简单脚本和快速任务 → v0
- 日常编码 → v1
- 复杂项目重构 → v2

### Q: 可以添加自己的工具吗？

A: 可以！每个版本的工具都是明确定义的。参考现有工具的实现，添加新的工具到 `create_tools()` 函数。

### Q: 支持其他 LLM 提供商吗？

A: 目前仅支持 Anthropic Claude。但架构是通用的，可以适配其他提供商。

### Q: 如何调试工具调用？

A: 所有版本都会打印工具调用和结果。查看输出中的 `> tool_name` 行。

## 总结

这个项目展示了构建 AI 编码代理的演进过程：

1. **v0**: 证明最小可行性（1 个工具）
2. **v1**: 添加实用工具（4 个工具）
3. **v2**: 增加规划能力（5 个工具 + 可见性）

关键洞察：代理的复杂性来自工具，而非模型。模型本身就是决策引擎。

从 ~200 行代码开始，逐步构建你自己的编码代理！
