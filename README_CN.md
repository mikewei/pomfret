<p align="center">
  <img src="static/pomfret.png" alt="Pomfret Logo" width="128" />
</p>

<h1 align="center">Pomfret</h1>

<p align="center">
  <strong>Proxy Of Models For Routing, Evaluation & Telemetry</strong>
</p>

<p align="center">
  一个灵活、轻量的 LLM 网关，让你轻松切换模型、智能路由请求，并监控 AI 技术栈中的每一个 Prompt。
</p>

<p align="center">
  <a href="https://github.com/mikewei/pomfret/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <a href="https://github.com/mikewei/pomfret/releases"><img src="https://img.shields.io/github/v/release/mikewei/pomfret" alt="Release" /></a>
</p>

<p align="center">
  <a href="README.md">English</a>
</p>

---

## 为什么选择 Pomfret？

使用多个 LLM 提供商不应该是一件痛苦的事。无论你是在评估模型、使用 [OpenClaw](https://github.com/ArcadeAI/OpenClaw) 构建 Agent，还是只想在不修改应用代码的情况下切换后端，**Pomfret** 都能充当你的客户端和 LLM 后端之间的桥梁，提供一个统一的 OpenAI 兼容端点。

- **一个端点，多个后端** — 将应用指向 Pomfret，即可在 OpenAI、Google Gemini、Ollama 或任何 OpenAI 兼容服务之间秒级切换。
- **智能路由** — 按模型名称、Prompt 长度或正则匹配路由请求。支持轮询负载均衡或锁定到特定后端。
- **全面可观测** — 内置 Web 控制台，可检查每一个请求和响应，轻松浏览 JSON 请求体和 Prompt 内容，追踪 Token 用量，实时监控后端健康状态。
- **零运行时依赖** — 编译为单一静态二进制文件，Web 控制台内嵌其中。无需 Node.js、Docker 或数据库。

## 功能一览

| 类别 | 详情 |
|---|---|
| **OpenAI 兼容 API** | `POST /v1/chat/completions`（流式和非流式）、`GET /v1/models` |
| **后端支持** | Ollama、OpenAI、Google Gemini 及任何 OpenAI 兼容的提供商（Azure OpenAI、Groq、Together AI 等） |
| **条件路由** | 基于规则的路由：按模型名称、请求体长度或 Prompt 内容的正则匹配 |
| **路由策略** | 首个可用、轮询，或锁定到特定后端 |
| **Web 控制台** | 配置管理、实时图表仪表盘、请求检查 — 一站式搞定 |
| **仪表盘** | 实时请求计数、Token 用量（Prompt / Completion），以及按后端统计的连接状态 |
| **请求检查** | 每次代理调用的完整请求与响应 JSON 体、Prompt 分析、模型信息、后端信息、延迟和状态 |
| **国际化** | Web 控制台支持英文和简体中文，根据浏览器语言自动切换 |
| **单一二进制** | 静态资源通过 `rust-embed` 编译到二进制中 — 只需部署一个文件 |

## 快速开始

### 安装

**从 GitHub Releases 下载**（推荐）

从 [Releases](https://github.com/mikewei/pomfret/releases) 页面下载适合你平台的预编译二进制文件。支持以下平台：

- macOS（Apple Silicon 和 Intel）
- Linux（aarch64 和 x86_64）
- Windows（x86_64）

或使用一键安装脚本：

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/mikewei/pomfret/releases/latest/download/pomfret-installer.sh | sh
```

**从源码构建**

```bash
git clone https://github.com/mikewei/pomfret.git
cd pomfret
cargo build --release
# 二进制文件在 target/release/pomfret
```

### 运行

```bash
pomfret
```

默认监听 `127.0.0.1:8080`。在浏览器打开 `http://localhost:8080/console` 进入 Web 控制台。

### 将客户端指向 Pomfret

在 OpenAI SDK、Agent 框架或任何兼容客户端中使用 `http://localhost:8080/v1` 作为 Base URL：

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3",
    "messages": [{"role": "user", "content": "你好！"}]
  }'
```

**配合 OpenClaw 使用** — 在 `openclaw.conf` 的 `models.providers` 中添加 Pomfret 作为提供商：

```json
{
  "pomfret": {
    "baseUrl": "http://localhost:8080/v1",
    "apiKey": "anything",
    "api": "openai-completions",
    "authHeader": false,
    "models": [
      {
        "id": "qwen3.5:9b",
        "name": "qwen3.5:9b",
        "api": "openai-completions",
        "reasoning": true,
        "input": ["text"],
        "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0 },
        "contextWindow": 65536,
        "maxTokens": 65536
      }
    ]
  }
}
```

Web 控制台还提供一键生成 OpenClaw 配置的功能 — 在 Configuration 页面点击 **OpenClaw Config** 按钮即可。

## 配置

Pomfret 可通过命令行参数或 TOML 配置文件（默认 `~/.pomfret/backends.conf`）进行配置。

| 参数 | 简写 | 说明 | 默认值 |
|---|---|---|---|
| `--config` | `-c` | 后端配置文件路径 | `~/.pomfret/backends.conf` |
| `--port` | `-p` | 监听端口 | `8080` |
| `--bind` | `-b` | 监听地址 | `127.0.0.1` |

所有后端和路由配置均可直接在 **Web 控制台** 中管理 — 添加、编辑或删除 LLM 后端，设置基于条件的路由规则（按模型名称、Prompt 长度或正则表达式），全部无需重启服务。

### 网络代理

访问上游 LLM 的出站请求使用 [reqwest](https://github.com/seanmonstar/reqwest)，会遵循常见的代理环境变量（与 curl 类似）。常用变量如下：

| 变量 | 作用 |
|---|---|
| `https_proxy` / `HTTPS_PROXY` | HTTPS 代理，适用于多数云端 HTTPS API |
| `http_proxy` / `HTTP_PROXY` | HTTP 代理 |
| `all_proxy` / `ALL_PROXY` | 同时作用于 HTTP 与 HTTPS |
| `no_proxy` / `NO_PROXY` | 逗号分隔的主机或网段，**不走代理**（例如本地 Ollama） |

示例：API 流量走本机代理，本地 Ollama 直连。下面为 **Linux / macOS** 下的 shell 写法（`export`）；在 Windows 上请用命令提示符的 `set` 或 PowerShell 的 `$env:...` 设置同名环境变量。

```bash
export https_proxy=http://127.0.0.1:7890
export no_proxy=127.0.0.1,localhost,.local
pomfret
```

不需要代理时，不设置或取消这些环境变量即可。

## 技术栈

- **后端**：Rust + [Axum](https://github.com/tokio-rs/axum) — 异步、零开销抽象、线程安全并发
- **HTTP 客户端**：[reqwest](https://github.com/seanmonstar/reqwest) + rustls — 支持 SSE 流式传输
- **前端**：原生 JavaScript + CSS — 无框架、无构建步骤
- **打包**：[rust-embed](https://github.com/pyrossh/rust-embed) 将 Web 控制台编译到二进制文件中
- **发布**：[cargo-dist](https://github.com/axodotdev/cargo-dist) 跨平台构建发布

## 开发

```bash
# 开发模式运行
cargo run

# 运行测试
cargo test

# 构建优化的发布版本
cargo build --release
```

设置 `RUST_LOG=pomfret=debug` 可开启详细日志。

## 开源协议

Pomfret 是开源软件，采用 [MIT License](LICENSE) 授权。
