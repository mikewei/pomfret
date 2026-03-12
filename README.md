# Pomfret

**Pomfret** = *Proxy Of Models For Routing, Evaluation & Telemetry*

OpenAI 兼容的 LLM 网关：对外提供统一 API，向后转发到 OpenAI 兼容服务或 Ollama；提供 Web 控制台查看请求与选择后端。

## 功能

- **OpenAI 兼容 API**：`POST /v1/chat/completions`（流式/非流式）、`GET /v1/models`
- **Web 控制台**：请求记录列表与详情（提示词、请求/响应体）、后端选择
- **后端选择**：在控制台选择当前转发的 LLM 后端（Ollama、OpenAI 等）

## 运行

```bash
cargo run
```

默认监听 `http://0.0.0.0:8080`。

- 健康检查：`GET /health`
- 代理 API：`POST /v1/chat/completions`、`GET /v1/models`
- 控制台：浏览器打开 `http://localhost:8080/console`

## 配置

配置优先级：**命令行 > 配置文件 > 默认值**。

- **命令行**：`-c/--config` 指定配置文件路径，`-p/--port` 指定端口，`-b/--bind` 指定监听地址。
- **配置文件**：默认路径为 `~/.pomfret/pomfret.conf`（TOML 格式）；未指定 `-c` 时会自动尝试该路径。
- **默认**：监听 `0.0.0.0:8080`，内置默认后端仅一条（Ollama，名称为 "Ollama"）。

示例：

```bash
cargo run                          # 使用默认；若存在 ~/.pomfret/pomfret.conf 则加载
cargo run -- -p 3000               # 端口改为 3000
cargo run -- -c ./my.conf -b 127.0.0.1
```

配置文件示例（`~/.pomfret/backends.conf`）。注意：**name** 为显示名称，**backend_type** 为后端类型（`ollama` 或 `openai_compat`），二者不同。

```toml
bind = "0.0.0.0"
port = 8080
current_index = 0

[[backends]]
id = "ollama"
name = "Ollama"
base_url = "http://127.0.0.1:11434"
backend_type = "ollama"

[[backends]]
id = "openai"
name = "OpenAI"
base_url = "https://api.openai.com"
api_key = "sk-..."
backend_type = "openai_compat"
```

在控制台「后端设置」中可切换当前使用的后端。

## 测试

```bash
cargo test
```

## 技术栈

- Rust + Axum
- 前端：原生 JavaScript，无框架；静态资源通过 `rust-embed` 编译进单一二进制

## License

本项目采用 [MIT License](LICENSE) 开源。
