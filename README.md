# Mini Claude Code - 编码代理的渐进式实现

一个通过五个版本逐步演进的项目，展示如何构建一个功能完整的 AI 编码代理。每个版本都建立在前一个版本的基础上，逐步添加功能和复杂性。

## 项目概览

这是一个 Rust workspace 项目，包含五个独立的 crate，代表编码代理的五个演进阶段：

- **v0_bash_agent**: 极简代理 - 仅使用 bash 工具
- **v1_basic_agent**: 基础代理 - 添加 4 个核心工具
- **v2_todo_agent**: 高级代理 - 增加任务规划和可见性
- **v3_subagent**: 子代理机制 - 通过上下文隔离解决复杂任务
- **v4_skills_agent**: 技能机制 + 网络搜索 - 知识外部化 + DuckDuckGo 搜索
- **v5_mcp_agent**: MCP 浏览器集成 - 通过 Chrome DevTools 实现浏览器自动化

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

## 版本对比

| 特性 | v0 | v1 | v2 | v3 | v4 | v5 |
|------|----|----|----|----|----|----|
| 工具数量 | 1 | 4 | 5 | 6 | 7 | 11 |
| 代码行数 | ~200 | ~500 | ~850 | ~1400 | ~1700 | ~2050 |
| 子代理支持 | ✅ 进程级 | ❌ | ❌ | ✅ 上下文隔离 | ✅ 上下文隔离 | ✅ 上下文隔离 |
| 任务规划 | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ |
| 技能加载 | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| **网络搜索** | ❌ | ❌ | ❌ | ❌ | **✅ DuckDuckGo** | **✅ DuckDuckGo** |
| **浏览器自动化** | ❌ | ❌ | ❌ | ❌ | ❌ | **✅ MCP + Chrome** |
| 适用场景 | 快速原型 | 日常编码 | 复杂重构 | 超大型任务 | 需要专业知识 | 需要浏览器交互 |

## 快速开始

### 环境要求

- Rust 1.70+ (建议使用 stable)
- API Key for Anthropic Claude
- (v5) Node.js 20+ + Chrome/Edge 浏览器

### 安装

```bash
# 克隆项目
git clone <repo-url>
cd learn-claude-code-rs

# 复制环境变量配置
cp .env.example .env

# 编辑 .env 文件，添加你的 API Key
# ANTHROPIC_API_KEY=your_key_here
```

### 运行各个版本

#### v0 - 极简代理 (~200 行)

```bash
# 交互模式
cargo run --bin v0_bash_agent

# 子代理模式（一次性执行任务）
cargo run --bin v0_bash_agent -- "分析 src/ 目录结构"
```

#### v1 - 基础代理 (~500 行)

```bash
cargo run --bin v1_basic_agent
```

#### v2 - 高级代理 (~850 行)

```bash
cargo run --bin v2_todo_agent
```

#### v3 - 子代理机制 (~1400 行)

```bash
cargo run -p v3_subagent
```

#### v4 - 技能 + 网络搜索 (~1700 行)

```bash
cargo run -p v4_skills_agent
```

**特性**:
- ✅ 技能加载（PDF、MCP、代码审查等）
- ✅ **DuckDuckGo 网络搜索**
- ✅ 子代理技能支持

**使用搜索**:
```
You: 搜索最新的 Rust async 编程最佳实践

> web_search "Rust async programming best practices 2025"
  返回 5 条搜索结果...
```

#### v5 - MCP 浏览器集成 (~2050 行) ⭐ 最新

```bash
# 1. 安装 chrome-devtools-mcp
npm install -g chrome-devtools-mcp

# 2. 启动 Chrome（带远程调试）
# Windows:
"C:\Program Files\Google\Chrome\Application\chrome.exe" --remote-debugging-port=9222

# Linux/macOS:
google-chrome --remote-debugging-port=9222
# 或 headless 模式:
google-chrome --headless --remote-debugging-port=9222

# 3. 运行程序
cargo run -p v5_mcp_agent
```

**新增浏览器工具**:
- `browser_navigate` - 导航到 URL
- `browser_screenshot` - 截取页面截图
- `browser_snapshot` - 获取页面文本内容（基于 a11y 树）
- `browser_get_performance` - 获取性能指标

**使用示例**:
```
You: 访问 https://example.com 并截图

> browser_navigate https://example.com
  ✅ 已导航到 https://example.com

> browser_screenshot
  ✅ 截图已完成

> browser_snapshot
  ✅ 页面内容:
  Example Domain
  This domain is for use in illustrative examples...
```

**完整工具列表**:
- bash, read_file, write_file, edit_file
- TodoWrite, Task, Skill
- web_search
- **browser_navigate, browser_screenshot, browser_snapshot, browser_get_performance**

## 版本详解

### v4: 技能机制 + 网络搜索

**核心哲学**: "技能即知识" + "互联网是最大的知识库"

**新增内容**:
- Skill 工具：按需加载专业领域知识
- **web_search 工具：DuckDuckGo 搜索集成**
- SkillLoader：解析 SKILL.md 文件
- 渐进式披露架构

**技能结构**:

```
skills/
├── pdf/
│   ├── SKILL.md          # PDF 处理技能
│   └── scripts/
├── mcp-builder/
│   └── SKILL.md          # MCP 服务器开发技能
└── code-review/
    └── SKILL.md          # 代码审查技能
```

**web_search 工具**:

使用 DuckDuckGo HTML 抓取实现免费搜索，无需 API Key。

```rust
// 搜索参数
{
  "query": "搜索关键词",
  "max_results": 5  // 可选，默认 5
}
```

**搜索策略**:
1. 解析 DuckDuckGo HTML 响应
2. 提取真实 URL（去除重定向）
3. 去重并限制结果数量
4. 返回标题、URL、摘要

**示例**:
```
You: 搜索 Rust MCP 协议实现

> web_search
  ## Search Results for: Rust MCP 协议实现

  1. **Model Context Protocol**
     URL: https://modelcontextprotocol.io/
     MCP 官方协议规范...

  2. **rust-mcp on GitHub**
     URL: https://github.com/...
     Rust MCP 实现示例...
```

**何时使用**:
- 需要最新信息（超出模型训练数据）
- 查找文档、教程、API 参考
- 技术问题排查
- 竞品调研

### v5: MCP 浏览器集成

**核心哲学**: "浏览器是最强大的数据采集工具"

**新增内容**:
- MCP Client：连接到 chrome-devtools-mcp
- 4 个浏览器工具（navigate, screenshot, snapshot, performance）
- Windows/Linux 跨平台支持
- JSON-RPC 通信协议

**架构**:

```
┌─────────────────────────────────────────────┐
│            v5_mcp_agent (Rust)              │
│  ┌──────────────┐  ┌───────────────────┐   │
│  │ Claude AI    │  │  MCP Client       │   │
│  │  (决策)      │─→│  (浏览器控制)    │   │
│  └──────────────┘  └───────────────────┘   │
└─────────────────────────────────────────────┘
                      │ JSON-RPC
                      ↓
┌─────────────────────────────────────────────┐
│      chrome-devtools-mcp (Node.js)          │
│         Chrome DevTools Protocol            │
└─────────────────────────────────────────────┘
                      │ CDP
                      ↓
┌─────────────────────────────────────────────┐
│           Chrome/Edge Browser               │
└─────────────────────────────────────────────┘
```

**MCP 工具详解**:

| 工具 | 功能 | MCP 对应工具 |
|------|------|--------------|
| browser_navigate | 访问 URL | navigate_page |
| browser_screenshot | 截图 | take_screenshot |
| browser_snapshot | 获取页面内容 | take_snapshot |
| browser_get_performance | 性能分析 | performance_start_trace + performance_stop_trace |

**跨平台支持**:

代码自动检测操作系统并适配：
- **Windows**: 通过 `cmd /C` 执行 npx
- **Linux/macOS**: 直接执行 npx

```rust
let command = if cfg!(target_os = "windows") {
    Command::new("cmd").args(["/C", "npx", ...])
} else {
    Command::new("npx").args([...])
};
```

**使用场景**:

1. **网页数据采集**
```
You: 访问新闻网站并提取标题

> browser_navigate https://news.example.com
> browser_snapshot
  提取到 10 条新闻标题...
```

2. **性能监控**
```
You: 分析网站加载性能

> browser_navigate https://yoursite.com
> browser_get_performance
  FCP: 1.2s, LCP: 2.3s, CLS: 0.05
```

3. **自动化测试**
```
You: 测试登录流程

> browser_navigate https://app.example.com/login
> browser_snapshot
  检测到登录表单，元素 ID: uid-123, uid-456
> (继续填写表单...)
```

**配置要求**:

1. **安装 chrome-devtools-mcp**:
```bash
npm install -g chrome-devtools-mcp
```

2. **启动 Chrome**:
```bash
# 带界面
chrome --remote-debugging-port=9222

# 无界面（headless）
chrome --headless --remote-debugging-port=9222 --no-sandbox
```

3. **验证连接**:
```bash
curl http://localhost:9222/json/version
```

应该看到 JSON 响应，包含 Chrome 版本信息。

**故障排查**:

如果遇到 "chrome-devtools-mcp not found"：
- 检查 Node.js 版本（需要 20+）
- 确认 npm 全局路径在 PATH 中
- Windows 用户：确保使用 PowerShell 或 CMD

如果遇到 "Target closed" 错误：
- 重启 Chrome（带 --remote-debugging-port=9222）
- 重启 v5_mcp_agent 程序
- 检查端口 9222 是否被占用

**何时使用 v5**:
- 需要从网页提取数据
- 需要与 Web 应用交互
- 需要测试前端性能
- 需要自动化浏览器任务
- 需要获取动态渲染的内容（AJAX、SPA）

## 环境变量

创建 `.env` 文件（参考 `.env.example`）：

```bash
# 必需：API Key
ANTHROPIC_API_KEY=your_api_key_here

# 可选：自定义 API Base URL
ANTHROPIC_API_BASE=https://api.anthropic.com

# 可选：模型选择
MODEL_NAME=claude-sonnet-4-5-20250929

# 可选：最大输出 tokens
MINI_CODE_MAX_OUTPUT_TOKENS=160000

# 可选：截断重试次数
MINI_CODE_MAX_TRUNCATION_RETRIES=3
```

支持的模型：
- `claude-sonnet-4-5-20250929` (更快，推荐)
- `claude-opus-4-5-20251101` (更强大)

## 测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定版本的测试
cargo test -p v4_skills_agent
cargo test -p v5_mcp_agent

# v2 和 v3 的环境变量测试需要单线程运行
cargo test -p v2_todo_agent -- --test-threads=1
cargo test -p v3_subagent -- --test-threads=1
```

## 使用示例

### 示例 1: 网络搜索 + 代码生成 (v4)

```
You: 搜索 Rust tokio select 用法并给我写个示例

> web_search "Rust tokio select usage example"
  [搜索结果...]

> write_file examples/tokio_select.rs
  [生成代码...]

完成！示例已保存到 examples/tokio_select.rs
```

### 示例 2: 浏览器自动化 (v5)

```
You: 访问 GitHub trending 页面并提取前 5 个项目

> browser_navigate https://github.com/trending
  ✅ 已导航

> browser_snapshot
  ✅ 获取页面内容

提取到的项目:
1. project-a/repo-name - 123 stars today
2. project-b/repo-name - 98 stars today
...
```

### 示例 3: 搜索 + 浏览器验证 (v5)

```
You: 搜索最新的 Next.js 文档，访问官网并截图

> web_search "Next.js official documentation"
  找到: https://nextjs.org/docs

> browser_navigate https://nextjs.org/docs
  ✅ 已导航

> browser_screenshot
  ✅ 截图已保存

> browser_snapshot
  文档包含以下章节: Getting Started, Routing, ...
```

## 性能考虑

- **超时**: 默认 10 分钟超时保护（可通过环境变量配置）
- **输出截断**: 工具输出限制在 50KB
- **UTF-8 安全**: 字符边界对齐的截断
- **路径验证**: 防止访问工作区外的文件
- **MCP 延迟**: 每次浏览器操作约 50-200ms

## 安全特性

- **路径验证**: 所有文件操作限制在工作区内
- **危险命令拦截**: 阻止 `rm -rf /` 等危险操作
- **进程隔离**: v0 的子代理在独立进程中运行
- **只读默认**: 文件操作需要明确的工具调用
- **浏览器沙箱**: Chrome 运行在受限环境

## 架构设计

### 工作空间结构

```
learn-claude-code-rs/
├── Cargo.toml                 # Workspace 配置
├── crates/
│   ├── v0_bash_agent/        # 极简版本
│   ├── v1_basic_agent/       # 基础版本
│   ├── v2_todo_agent/        # 高级版本
│   ├── v3_subagent/          # 子代理版本
│   ├── v4_skills_agent/      # 技能 + 搜索版本
│   └── v5_mcp_agent/         # MCP 浏览器版本
│       ├── src/
│       │   ├── main.rs       # 主程序
│       │   └── mcp_client.rs # MCP 客户端
│       └── README.md
├── skills/                   # 技能目录
│   ├── pdf/
│   ├── mcp-builder/
│   ├── code-review/
│   └── agent-builder/
├── .env.example
└── README.md
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

## 学习路径

建议按顺序阅读和运行每个版本：

1. **v0**: 理解代理的基本循环（~200 行代码）
2. **v1**: 体验更多工具的便利（+300 行）
3. **v2**: 尝试复杂任务的规划（+350 行）
4. **v3**: 体验子代理的上下文隔离（+550 行）
5. **v4**: 使用技能和搜索功能（+300 行）
6. **v5**: 探索浏览器自动化（+350 行）

每个版本都是完整可用的，可以独立运行。

## 贡献

欢迎贡献！特别是：

- 添加新的工具实现
- 改进 MCP 集成
- 添加更多技能
- 增加测试覆盖
- 优化性能
- 改进文档

## 许可证

MIT License

## 致谢

本项目受到以下项目的启发：
- Anthropic Claude Code
- Model Context Protocol (MCP)
- Chrome DevTools Protocol
- Cursor AI

## 总结

这个项目展示了构建 AI 编码代理的演进过程：

1. **v0**: 证明最小可行性（1 个工具，~200 行）
2. **v1**: 添加实用工具（4 个工具，~500 行）
3. **v2**: 增加规划能力（5 个工具 + 可见性，~850 行）
4. **v3**: 上下文隔离（6 个工具 + 子代理，~1400 行）
5. **v4**: 知识外部化 + 网络搜索（7 个工具，~1700 行）
6. **v5**: 浏览器自动化（11 个工具，~2050 行）

关键洞察：代理的复杂性来自工具，而非模型。模型本身就是决策引擎。

从 ~200 行代码开始，逐步构建你自己的编码代理！🚀
