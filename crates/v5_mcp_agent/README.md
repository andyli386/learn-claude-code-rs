# v5_mcp_agent - MCP 浏览器自动化代理

## 🎯 概述

v5_mcp_agent 在 v4_skills_agent 的基础上添加了 **MCP (Model Context Protocol)** 浏览器自动化支持，使得 AI Agent 能够通过 Chrome DevTools Protocol 控制浏览器，实现网页数据采集、性能分析、自动化测试等功能。

## ✨ 新增功能

### 浏览器自动化工具

| 工具 | 功能 | 用途 |
|------|------|------|
| `browser_navigate` | 导航到 URL | 访问网页 |
| `browser_screenshot` | 截图 | 保存页面视觉效果 |
| `browser_snapshot` | 获取页面文本内容 | 提取数据（基于 a11y 树）|
| `browser_get_performance` | 性能分析 | 获取 FCP、LCP、CLS 等指标 |

### 继承自 v4 的功能

- ✅ **所有 v4 功能**（7 个工具 + 技能系统）
- ✅ **web_search**：DuckDuckGo 搜索
- ✅ **Skill 系统**：按需加载专业知识
- ✅ **TodoWrite**：任务规划
- ✅ **Task**：子代理隔离

## 🚀 快速开始

### 前置要求

1. **Node.js 20+**
   ```bash
   node --version  # 应该 >= v20
   ```

2. **Chrome 或 Edge 浏览器**

3. **chrome-devtools-mcp**
   ```bash
   npm install -g chrome-devtools-mcp

   # 验证安装
   npx -y chrome-devtools-mcp@latest --version
   ```

### 启动步骤

#### Windows

```powershell
# 1. 启动 Chrome（在一个 PowerShell 窗口）
"C:\Program Files\Google\Chrome\Application\chrome.exe" --remote-debugging-port=9222 --user-data-dir="%TEMP%\chrome-debug"

# 2. 验证 Chrome 远程调试（在浏览器中访问）
# http://localhost:9222/json/version

# 3. 运行程序（在另一个 PowerShell 窗口）
cd E:\learn-claude-code-rs
cargo run -p v5_mcp_agent --release
```

#### Linux/macOS

```bash
# 1. 启动 Chrome（带界面）
google-chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-debug &

# 或无界面模式（headless）
google-chrome --headless --remote-debugging-port=9222 --no-sandbox --user-data-dir=/tmp/chrome-debug &

# 2. 验证连接
curl http://localhost:9222/json/version

# 3. 运行程序
cargo run -p v5_mcp_agent
```

### WSL 环境

如果在 WSL 中运行，推荐直接在 WSL 中安装 Chrome：

```bash
# 安装 Chrome
wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
sudo apt-get install ./google-chrome-stable_current_amd64.deb

# 启动 Chrome（headless）
google-chrome --headless --remote-debugging-port=9222 --no-sandbox &

# 运行程序
cargo run -p v5_mcp_agent
```

## 💡 使用示例

### 示例 1: 访问网页并截图

```
You: 访问 https://example.com 并截图

Agent:
> browser_navigate
  ✅ 导航到 https://example.com

> browser_screenshot
  ✅ 截图已保存

> browser_snapshot
  ✅ 页面内容:
  Example Domain
  This domain is for use in illustrative examples in documents...
```

### 示例 2: 性能分析

```
You: 分析 https://github.com 的性能

Agent:
> browser_navigate
  ✅ 导航到 https://github.com

> browser_get_performance
  📊 Performance Metrics:
  - FCP (First Contentful Paint): 1.2s
  - LCP (Largest Contentful Paint): 2.1s
  - TTI (Time to Interactive): 3.5s
  - CLS (Cumulative Layout Shift): 0.05
```

### 示例 3: 数据采集

```
You: 访问 Hacker News 首页并提取前 5 条新闻标题

Agent:
> browser_navigate
  ✅ 导航到 https://news.ycombinator.com

> browser_snapshot
  ✅ 获取页面内容

分析完成，找到以下标题:
1. Show HN: I built a tool for...
2. Ask HN: How do you...
3. New programming language...
...
```

### 示例 4: 搜索 + 浏览器验证

```
You: 搜索 Rust 官网并访问文档页面

Agent:
> web_search "Rust official website"
  找到: https://www.rust-lang.org

> browser_navigate https://www.rust-lang.org
  ✅ 已导航

> browser_snapshot
  页面包含: Learn Rust, Get Started, Documentation...

> browser_navigate https://doc.rust-lang.org/book/
  ✅ 已导航到《Rust 程序设计语言》
```

## 🏗️ 架构

```
┌─────────────────────────────────────────────────────────────┐
│                      v5_mcp_agent (Rust)                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Claude AI    │  │   Tools      │  │   Skills     │      │
│  │   Client     │──│ (bash, file, │──│   (pdf,      │      │
│  │              │  │  browser,    │  │  mcp, etc.)  │      │
│  │              │  │  search)     │  │              │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                          │                                   │
│                          │ MCP Client (mcp_client.rs)       │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ JSON-RPC over stdio
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              chrome-devtools-mcp (Node.js)                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ navigate_page│  │take_screenshot│  │ take_snapshot│      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ Chrome DevTools Protocol (CDP)
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Chrome/Edge Browser                       │
│                  (--remote-debugging-port=9222)              │
└─────────────────────────────────────────────────────────────┘
```

## 🔧 工具映射

v5_mcp_agent 的浏览器工具映射到 `chrome-devtools-mcp` 的实际工具：

| v5 工具 | MCP 工具 | 参数 |
|---------|----------|------|
| `browser_navigate` | `navigate_page` | `{type: "url", url: "..."}` |
| `browser_screenshot` | `take_screenshot` | `{}` |
| `browser_snapshot` | `take_snapshot` | `{verbose: false}` |
| `browser_get_performance` | `performance_start_trace` + `performance_stop_trace` | 组合调用 |

## 📝 实现细节

### MCP 通信流程

1. **启动 MCP 服务器**（程序启动时自动）
   ```rust
   let mcp_process = Command::new("npx")
       .args(["-y", "chrome-devtools-mcp@latest"])
       .env("CHROME_REMOTE_DEBUGGING_URL", "http://localhost:9222")
       .spawn()?;
   ```

2. **发送 JSON-RPC 请求**
   ```json
   {
     "jsonrpc": "2.0",
     "method": "tools/call",
     "params": {
       "name": "navigate_page",
       "arguments": {"type": "url", "url": "https://example.com"}
     },
     "id": 1
   }
   ```

3. **解析响应**
   ```json
   {
     "jsonrpc": "2.0",
     "result": {
       "content": [{"type": "text", "text": "Navigated successfully"}]
     },
     "id": 1
   }
   ```

### 跨平台支持

代码自动检测操作系统并适配命令执行方式：

```rust
let output = if cfg!(target_os = "windows") {
    // Windows: 通过 cmd.exe 执行
    Command::new("cmd")
        .args(["/C", "npx", "-y", "chrome-devtools-mcp@latest", "--version"])
        .output()
} else {
    // Linux/macOS: 直接执行
    Command::new("npx")
        .args(["-y", "chrome-devtools-mcp@latest", "--version"])
        .output()
};
```

## ⚠️ 注意事项

### 安全性

- ⚠️ **浏览器内容会发送给 AI 模型**，不要在浏览器中打开敏感页面
- ⚠️ **MCP 通信未加密**，不要在不安全的网络环境中使用
- ⚠️ **建议使用专用的 Chrome 配置文件**（`--user-data-dir`）

### 性能

- 每次 MCP 调用有约 **50-200ms** 的延迟
- 截图和性能分析会消耗更多资源
- 建议批量操作以减少往返次数

### 兼容性

- 需要 **Chrome/Edge 稳定版**（最新版本）
- `chrome-devtools-mcp` 需要 **Node.js 20+**
- 在 **WSL** 上可能需要安装额外的依赖（如 libgbm1）

## 🐛 故障排查

### 问题 1: "chrome-devtools-mcp not found"

**原因**: npx 命令找不到或环境变量配置问题

**解决方案**:
```bash
# 检查 Node.js 和 npm
node --version  # 应该 >= v20
npm --version

# 检查 npx
npx --version

# 手动安装
npm install -g chrome-devtools-mcp

# 验证安装
npx -y chrome-devtools-mcp@latest --version
```

**Windows 特定问题**:
```powershell
# 检查 npm 全局路径是否在 PATH 中
npm config get prefix
# 应该返回类似: C:\Users\YourName\AppData\Roaming\npm

# 确保该路径在系统 PATH 中
$env:Path -split ';' | Select-String npm
```

### 问题 2: "Chrome not found" 或 "Target closed"

**原因**: Chrome 未启动或远程调试端口未开启

**解决方案**:
```bash
# 1. 检查 Chrome 是否在运行
# Windows:
tasklist | findstr chrome

# Linux/macOS:
ps aux | grep chrome

# 2. 检查端口 9222 是否监听
# Windows:
netstat -ano | findstr :9222

# Linux/macOS:
lsof -i :9222
# 或
netstat -tlnp | grep 9222

# 3. 访问远程调试端点
curl http://localhost:9222/json/version
# 或在浏览器中访问: http://localhost:9222

# 4. 如果没有响应，重启 Chrome
# 先杀掉所有 Chrome 进程
# Windows:
taskkill /F /IM chrome.exe

# Linux/macOS:
pkill -9 chrome

# 然后重新启动
chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-debug
```

### 问题 3: MCP 通信超时或无响应

**原因**: MCP 服务器启动失败或 Chrome 连接问题

**解决方案**:
```bash
# 1. 手动测试 MCP 服务器
npx -y chrome-devtools-mcp@latest
# 应该看到启动信息

# 2. 检查环境变量
echo $CHROME_REMOTE_DEBUGGING_URL  # Linux/macOS
echo %CHROME_REMOTE_DEBUGGING_URL% # Windows

# 3. 重启所有组件
# - 关闭 Chrome
# - 关闭 v5_mcp_agent
# - 重新启动 Chrome（带 --remote-debugging-port=9222）
# - 重新启动 v5_mcp_agent
```

### 问题 4: WSL 中 Chrome 启动失败

**原因**: 缺少图形界面库或依赖

**解决方案**:
```bash
# 安装必要的依赖
sudo apt-get update
sudo apt-get install -y \
    libgbm1 \
    libnss3 \
    libatk-bridge2.0-0 \
    libgtk-3-0 \
    libx11-xcb1 \
    libxcomposite1 \
    libxdamage1 \
    libxrandr2

# 使用 headless 模式（推荐）
google-chrome \
    --headless \
    --remote-debugging-port=9222 \
    --no-sandbox \
    --disable-gpu \
    --disable-dev-shm-usage \
    --user-data-dir=/tmp/chrome-debug
```

## 🔍 MCP 集成实战总结

### 正确的集成步骤（最佳实践）

根据实际开发经验，以下是推荐的步骤：

#### 方案 A: Windows 原生环境（推荐 ⭐）

```powershell
# 1. 确认环境
node --version    # >= v20
npm --version

# 2. 安装 MCP 服务器
npm install -g chrome-devtools-mcp
npx -y chrome-devtools-mcp@latest --version  # 验证

# 3. 启动 Chrome（新窗口）
"C:\Program Files\Google\Chrome\Application\chrome.exe" --remote-debugging-port=9222 --user-data-dir="%TEMP%\chrome-debug"

# 4. 验证 Chrome 连接（浏览器访问）
# http://localhost:9222/json/version

# 5. 运行程序
cargo run -p v5_mcp_agent --release
```

**为什么推荐这个方案？**
- ✅ 所有组件在同一环境，通信稳定
- ✅ `chrome-devtools-mcp` 在 Windows 上兼容性最好
- ✅ 调试方便，问题容易定位

#### 方案 B: WSL 环境（需要额外配置）

```bash
# 1. 在 WSL 中安装 Chrome
wget https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb
sudo apt-get install ./google-chrome-stable_current_amd64.deb

# 2. 安装必要依赖
sudo apt-get install -y libgbm1 libnss3 libatk-bridge2.0-0

# 3. 启动 Chrome（headless 模式）
google-chrome --headless --remote-debugging-port=9222 --no-sandbox --disable-gpu --user-data-dir=/tmp/chrome-debug &

# 4. 验证连接
curl http://localhost:9222/json/version

# 5. 运行程序
cargo run -p v5_mcp_agent
```

### 常见错误及解决方案总结

#### 错误 1: "chrome-devtools-mcp not found"

**问题现象**:
```
⚠ chrome-devtools-mcp not found
   Install with: npm install -g chrome-devtools-mcp
⚠ Browser tools will be unavailable
```

**根本原因**:
1. **Windows**: Rust 的 `Command::new("npx")` 无法直接执行，需要通过 `cmd /C`
2. **Linux**: npx 未安装或不在 PATH 中

**解决方案**:
```rust
// 代码已修复（v5_mcp_agent/src/mcp_client.rs）
let output = if cfg!(target_os = "windows") {
    Command::new("cmd").args(["/C", "npx", ...])
} else {
    Command::new("npx").args([...])
};
```

手动验证：
```powershell
# Windows
cmd /C npx -y chrome-devtools-mcp@latest --version

# Linux
npx -y chrome-devtools-mcp@latest --version
```

#### 错误 2: "Protocol error (Target.setDiscoverTargets): Target closed"

**问题现象**:
```
> browser_navigate
  Protocol error (Target.setDiscoverTargets): Target closed
Cause:
```

**根本原因**:
1. Chrome 未启动或远程调试端口未开启
2. **跨环境问题**：程序在 WSL，Chrome 在 Windows（无法通信）
3. Chrome 启动后重启了程序，MCP 连接到旧实例
4. Chrome 启动参数缺失 `--remote-debugging-port=9222`

**解决方案**:

**情况 1: 跨环境问题（WSL ↔ Windows）**
```bash
# ❌ 错误做法：程序在 WSL，Chrome 在 Windows
# WSL: cargo run -p v5_mcp_agent
# Windows: chrome.exe --remote-debugging-port=9222
# 结果：无法通信

# ✅ 正确做法 1：都在 Windows
# Windows PowerShell:
cargo run -p v5_mcp_agent
chrome.exe --remote-debugging-port=9222

# ✅ 正确做法 2：都在 WSL
# WSL:
cargo run -p v5_mcp_agent
google-chrome --headless --remote-debugging-port=9222
```

**情况 2: Chrome 端口未监听**
```bash
# 检查端口
# Windows:
netstat -ano | findstr :9222

# Linux:
lsof -i :9222

# 如果没有输出，说明 Chrome 没有正确启动
# 重新启动 Chrome（确保参数正确）
```

**情况 3: MCP 连接到旧实例**
```bash
# 解决方案：按顺序重启
# 1. 关闭 Chrome
taskkill /F /IM chrome.exe  # Windows
pkill -9 chrome             # Linux

# 2. 启动 Chrome
chrome --remote-debugging-port=9222

# 3. 等待 2-3 秒

# 4. 启动程序
cargo run -p v5_mcp_agent
```

#### 错误 3: WSL 中 Chrome 无法启动

**问题现象**:
```bash
google-chrome --remote-debugging-port=9222
# 错误：libgbm.so.1: cannot open shared object file
```

**根本原因**:
WSL 缺少图形界面库

**解决方案**:
```bash
# 安装依赖
sudo apt-get update
sudo apt-get install -y \
    libgbm1 \
    libnss3 \
    libatk-bridge2.0-0 \
    libgtk-3-0 \
    libx11-xcb1

# 使用 headless 模式（推荐）
google-chrome \
    --headless \
    --remote-debugging-port=9222 \
    --no-sandbox \
    --disable-gpu \
    --disable-dev-shm-usage \
    --user-data-dir=/tmp/chrome-debug
```

### 验证清单

在运行程序前，确保：

```bash
# ✅ 1. Chrome 正在运行并监听端口 9222
# Windows:
netstat -ano | findstr :9222
# Linux:
lsof -i :9222

# ✅ 2. 能访问远程调试端点
curl http://localhost:9222/json/version
# 应该返回 JSON 响应，包含 Browser、Protocol-Version 等

# ✅ 3. npx 可用
npx --version
npx -y chrome-devtools-mcp@latest --version

# ✅ 4. 环境一致性
# 确保 Chrome、MCP、程序在同一环境中（都在 Windows 或都在 WSL）
```

### 调试技巧

#### 1. 手动测试 MCP 服务器

```bash
# 启动 MCP 服务器（手动）
npx -y chrome-devtools-mcp@latest

# 应该看到：
# chrome-devtools-mcp exposes content of the browser instance...
# Avoid sharing sensitive or personal information...
```

如果启动失败，检查：
- Node.js 版本
- Chrome 是否在运行
- 端口 9222 是否可访问

#### 2. 测试工具列表

```bash
# 创建测试脚本
cat > test_mcp.js << 'EOF'
const { spawn } = require('child_process');
const mcp = spawn('npx', ['-y', 'chrome-devtools-mcp@latest'], {
  env: { ...process.env, CHROME_REMOTE_DEBUGGING_URL: 'http://localhost:9222' },
  stdio: ['pipe', 'pipe', 'pipe']
});

setTimeout(() => {
  const request = {
    jsonrpc: "2.0",
    method: "tools/list",
    id: 1
  };
  mcp.stdin.write(JSON.stringify(request) + '\n');
}, 2000);

mcp.stdout.on('data', (data) => console.log('OUT:', data.toString()));
mcp.stderr.on('data', (data) => console.log('ERR:', data.toString()));

setTimeout(() => mcp.kill(), 5000);
EOF

node test_mcp.js
```

应该看到可用工具列表，包括 `navigate_page`、`take_screenshot` 等。

#### 3. 检查程序启动日志

正确的启动应该显示：
```
✓ chrome-devtools-mcp detected
🌐 Starting chrome-devtools-mcp server...
✅ chrome-devtools-mcp server started
...
Browser: Browser automation enabled
```

如果显示 "Browser automation disabled"，说明检测失败。

### 最佳实践建议

1. **环境统一**: 优先在 Windows 原生环境运行（所有组件）
2. **独立 Chrome 配置**: 使用 `--user-data-dir` 避免与日常浏览冲突
3. **先验证再运行**: 先访问 `http://localhost:9222` 确认 Chrome 正常
4. **顺序启动**: Chrome → 验证 → 程序
5. **重启顺序**: 关闭程序 → 关闭 Chrome → 启动 Chrome → 启动程序

## 📚 相关资源

- [MCP 协议规范](https://modelcontextprotocol.io/)
- [chrome-devtools-mcp GitHub](https://github.com/modelcontextprotocol/servers/tree/main/src/chrome-devtools-mcp)
- [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/)
- [v5 集成文档](../../V5_BROWSER_INTEGRATION_COMPLETE.md)

## 🚧 未来改进

- [ ] 支持更多浏览器操作（点击、输入、等待）
- [ ] 支持多标签页管理
- [ ] Cookie 和会话管理
- [ ] 网络请求拦截和修改
- [ ] 持久化 MCP 连接（避免每次重启）
- [ ] 错误重试机制
- [ ] 完整的单元测试覆盖

## 📊 版本历史

- **v5.0.0** (2025-01-11)
  - ✅ 添加 MCP 浏览器支持
  - ✅ 新增 4 个浏览器工具
  - ✅ Windows/Linux 跨平台支持
  - ✅ 基础架构完成

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

特别欢迎：
- 更多浏览器工具实现
- 性能优化
- 错误处理改进
- 文档补充

## 📄 许可证

MIT License
