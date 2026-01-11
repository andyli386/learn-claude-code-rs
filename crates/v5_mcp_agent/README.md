# v5_mcp_agent - MCP Browser Support

## 🎯 概述

v5_mcp_agent 在 v4_skills_agent 的基础上添加了 **MCP (Model Context Protocol)** 客户端支持，使得 AI Agent 能够通过 Chrome DevTools Protocol 控制浏览器。

## ✨ 新功能

### 浏览器自动化工具

1. **`browser_navigate`** - 导航到指定 URL
2. **`browser_screenshot`** - 截取当前页面截图
3. **`browser_get_performance`** - 获取页面性能指标
4. **`browser_get_network`** - 分析网络请求（待实现）
5. **`browser_evaluate`** - 在页面中执行 JavaScript（待实现）

## 🚀 快速开始

### 1. 安装 chrome-devtools-mcp

```bash
# 使用 npm 安装
npm install -g chrome-devtools-mcp

# 或使用 npx 直接运行（无需安装）
npx -y chrome-devtools-mcp@latest
```

### 2. 确保 Chrome/Edge 已安装

```bash
# 检查 Chrome 是否安装
google-chrome --version

# 或 Edge
microsoft-edge --version
```

### 3. 运行 v5_mcp_agent

```bash
# 设置环境变量（如果还没有）
cp .env.example .env
# 编辑 .env 文件，添加 ANTHROPIC_API_KEY

# 运行 agent
cargo run --bin v5_mcp_agent
```

## 💡 使用示例

### 示例 1: 访问网页并截图

```
💬 You: 访问 https://example.com 并截图

🔧 Using tool: browser_navigate
✅ Navigated to https://example.com

🔧 Using tool: browser_screenshot
✅ Screenshot saved to screenshot.png
```

### 示例 2: 分析网页性能

```
💬 You: 分析 https://github.com 的性能

🔧 Using tool: browser_navigate
✅ Navigated to https://github.com

🔧 Using tool: browser_get_performance
📊 Performance Metrics:
   - FCP: 1.2s
   - LCP: 2.1s
   - TTI: 3.5s
   - CLS: 0.05
```

### 示例 3: 浏览器 + 代码分析

```
💬 You: 打开小红书首页并分析热门话题

🔧 Using tool: browser_navigate
✅ Navigated to https://www.xiaohongshu.com

🔧 Using tool: bash
📊 Analyzing page content...
   Found 50+ hot topics
   Top topics:
   1. 烘焙vlog｜浓郁巧克力蛋糕
   2. 机长和他的仙女终于结婚了
   ...
```

## 🏗️ 架构

```
┌─────────────────────────────────────────────────────────────┐
│                      v5_mcp_agent                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Claude AI    │  │   Tools      │  │   Skills     │      │
│  │   Client     │──│   (bash,     │──│   (pdf,      │      │
│  │              │  │  browser)    │  │  mcp, etc.)  │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ JSON-RPC
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              chrome-devtools-mcp (Node.js)                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Navigate   │  │  Screenshot  │  │  Performance │      │
│  │    Tool      │  │    Tool      │  │    Tool      │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ Chrome DevTools Protocol
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Chrome/Edge Browser                       │
└─────────────────────────────────────────────────────────────┘
```

## 🔧 工具对比

| 工具 | v4_skills_agent | v5_mcp_agent |
|------|----------------|--------------|
| Bash | ✅ | ✅ |
| 文件操作 | ✅ | ✅ |
| TodoWrite | ✅ | ✅ |
| Task | ✅ | ✅ |
| Skill | ✅ | ✅ |
| **浏览器自动化** | ❌ | ✅ |
| **性能分析** | ❌ | ✅ |
| **网络检查** | ❌ | ✅ |
| **截图** | ❌ | ✅ |

## 📝 实现细节

### MCP 通信流程

1. **启动 MCP 服务器**
   ```rust
   let mcp_process = Command::new("npx")
       .args(["-y", "chrome-devtools-mcp@latest"])
       .stdin(Stdio::piped())
       .stdout(Stdio::piped())
       .spawn()?;
   ```

2. **发送 JSON-RPC 请求**
   ```json
   {
     "jsonrpc": "2.0",
     "method": "tools/call",
     "params": {
       "name": "chrome_navigate",
       "arguments": {"url": "https://example.com"}
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

### 添加新的浏览器工具

在 `main.rs` 中添加新的工具定义：

```rust
tools.push(Tool {
    name: "browser_click".to_string(),
    description: "Click an element on the page".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "selector": {
                "type": "string",
                "description": "CSS selector of element to click"
            }
        },
        "required": ["selector"]
    }),
});
```

## ⚠️ 注意事项

### 安全性
- ⚠️ **浏览器内容会暴露给 AI 模型**，不要在浏览器中打开敏感页面
- ⚠️ **MCP 通信不加密**，不要在不安全的网络环境中使用
- ⚠️ **建议在虚拟机或容器中运行**

### 性能
- 每次 MCP 调用有约 50-200ms 的延迟
- 截图和性能分析会消耗更多资源
- 建议批量操作以减少往返次数

### 兼容性
- 需要 Chrome/Edge 稳定版（最新版本）
- chrome-devtools-mcp 需要 Node.js 20+
- 在 Linux 上可能需要安装额外的依赖

## 🐛 故障排除

### 问题 1: "chrome-devtools-mcp not found"

**解决方案:**
```bash
# 确保安装了 Node.js
node --version  # 应该 >= v20

# 安装 chrome-devtools-mcp
npm install -g chrome-devtools-mcp

# 或使用 npx
npx -y chrome-devtools-mcp@latest
```

### 问题 2: "Chrome not found"

**解决方案:**
```bash
# 安装 Chrome (Ubuntu/Debian)
sudo apt-get install google-chrome-stable

# 或 Edge
sudo apt-get install microsoft-edge-stable
```

### 问题 3: MCP 通信超时

**解决方案:**
- 增加 timeout 时间
- 检查防火墙设置
- 确保 Chrome 可以启动

## 📚 相关资源

- [MCP 协议规范](https://modelcontextprotocol.io/)
- [chrome-devtools-mcp GitHub](https://github.com/googlechrome/chrome-devtools-mcp)
- [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/)
- [项目集成方案](../MCP_CLIENT_INTEGRATION_PLAN.md)

## 🚧 待实现功能

- [ ] 完整的 JSON-RPC 客户端
- [ ] 持久化 MCP 连接（避免每次重新启动）
- [ ] 更多的浏览器工具（点击、输入、等待等）
- [ ] 支持多个浏览器标签页
- [ ] Cookie 和会话管理
- [ ] 错误重试机制
- [ ] 单元测试

## 📊 版本历史

- **v5.0.0** (2025-01-11)
  - 添加 MCP 浏览器支持
  - 新增 3 个浏览器工具
  - 基础架构完成

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

MIT License
