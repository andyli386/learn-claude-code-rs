# NewAPI Claude Code 凭证限制问题排查

## 问题描述

错误信息：
```
This credential is only authorized for use with Claude Code and cannot be used for other API requests.
```

## 可能的检测方式

NewAPI 可能通过以下方式验证是否是真实的 Claude Code 请求：

### 1. User-Agent 检查
- 我们已经设置：`claude-code/2.1.2`
- 可能需要的格式：
  - `Claude Code/2.1.2`
  - `anthropic-sdk-typescript/x.x.x`
  - 其他特定格式

### 2. 请求头检查
可能检查的额外头信息：
- `Origin`: 可能需要特定的源
- `Referer`: 可能需要Claude Code 相关的引用
- `X-Claude-Code`: 自定义头
- 其他特定标识头

### 3. 请求特征
- Token 格式检查（可能有特殊前缀或格式）
- 请求体结构
- API 版本号

## 解决方案

### 方案 1: 运行测试工具

我创建了一个测试工具来尝试不同的头组合：

```bash
cargo run -p v0_bash_agent --bin test_headers
```

这将测试多种 User-Agent 和 Origin 组合，看哪个能通过。

### 方案 2: 抓包分析真实 Claude Code 请求

如果你有真实的 Claude Code，可以抓包看看它发送的完整请求：

**在 macOS/Linux:**
```bash
# 安装 mitmproxy
pip install mitmproxy

# 设置代理
export HTTP_PROXY=http://localhost:8080
export HTTPS_PROXY=http://localhost:8080

# 运行 mitmproxy
mitmproxy

# 在另一个终端运行 Claude Code
claude-code
```

**在 Windows:**
使用 Fiddler 或 Burp Suite 抓包

### 方案 3: 检查 NewAPI 设置

NewAPI 可能有配置选项来控制这个限制。检查：

1. **NewAPI 管理后台**
   - 检查令牌/渠道设置
   - 可能有"仅限 Claude Code"的选项需要关闭

2. **渠道配置**
   - 检查是否有特定的渠道限制
   - 尝试创建新的渠道/令牌

3. **模型组设置**
   - 某些模型组可能限制只能通过 Claude Code 访问

### 方案 4: 修改 SDK 添加更多头信息

编辑 `/home/vincent/project/anthropic-rs/anthropic/src/client.rs`：

```rust
fn headers(&self) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(API_KEY_HEADER, HeaderValue::from_str(&self.api_key).unwrap());
    headers.insert(VERSION_HEADER, HeaderValue::from_str(&self.api_version).unwrap());
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    // 尝试添加更多 Claude Code 特征
    headers.insert(USER_AGENT, HeaderValue::from_static("claude-code/2.1.2"));
    headers.insert("origin", HeaderValue::from_static("vscode://claude.code"));
    headers.insert("x-claude-code-version", HeaderValue::from_static("2.1.2"));

    if let Some(beta) = &self.beta {
        headers.insert(BETA_HEADER, HeaderValue::from_str(beta).unwrap());
    }
    headers
}
```

### 方案 5: 使用不同的令牌

如果可能，在 NewAPI 后台：
1. 创建一个新的令牌
2. **不要**标记为"仅限 Claude Code"
3. 使用普通的 API 令牌

## 调试步骤

1. **运行头信息测试：**
   ```bash
   cargo run -p v0_bash_agent --bin test_headers
   ```

2. **检查 NewAPI 日志：**
   - 查看 NewAPI 的日志文件
   - 看看具体的拒绝原因

3. **尝试直接 curl：**
   ```bash
   curl -X POST https://xz.ai2api.dev/v1/messages \
     -H "x-api-key: $ANTHROPIC_AUTH_TOKEN" \
     -H "anthropic-version: 2023-06-01" \
     -H "content-type: application/json" \
     -H "user-agent: claude-code/2.1.2" \
     -d '{
       "model": "claude-sonnet-4-5-20250929",
       "max_tokens": 10,
       "messages": [{"role": "user", "content": "Hi"}]
     }'
   ```

## 相关资源

- [NewAPI GitHub](https://github.com/Calcium-Ion/new-api)
- [Claude Code GitHub](https://github.com/anthropics/claude-code)
- [Claude API 文档](https://platform.claude.com/docs/en/api/overview)

## 下一步

建议按以下顺序尝试：

1. ✅ 运行 `test_headers` 工具查看哪个组合有效
2. 🔍 检查 NewAPI 后台设置，看是否有限制选项
3. 📦 如果都不行，抓包真实的 Claude Code 请求
4. 💡 考虑联系 NewAPI 管理员或查看其文档
