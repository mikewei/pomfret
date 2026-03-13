<p align="center">
  <img src="static/pomfret.png" alt="Pomfret Logo" width="128" />
</p>

<h1 align="center">Pomfret</h1>

<p align="center">
  <strong>Proxy Of Models For Routing, Evaluation & Telemetry</strong>
</p>

<p align="center">
  A flexible, lightweight LLM gateway that makes it effortless to switch between models, route requests intelligently, and monitor every prompt that flows through your AI stack.
</p>

<p align="center">
  <a href="https://github.com/mikewei/pomfret/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" /></a>
  <a href="https://github.com/mikewei/pomfret/releases"><img src="https://img.shields.io/github/v/release/mikewei/pomfret" alt="Release" /></a>
</p>

<p align="center">
  <a href="README_CN.md">简体中文</a>
</p>

---

## Why Pomfret?

Working with multiple LLM providers shouldn't be painful. Whether you're evaluating models, building agents with [OpenClaw](https://github.com/ArcadeAI/OpenClaw), or just want to swap backends without touching your application code, **Pomfret** sits between your client and the LLM backends, giving you a single, unified OpenAI-compatible endpoint.

- **One endpoint, many backends** — point your app at Pomfret and switch between OpenAI, Ollama, or any OpenAI-compatible service in seconds.
- **Smart routing** — route requests by model name, prompt length, or regex patterns. Load-balance with round-robin or pin to a specific backend.
- **Full observability** — a built-in web console lets you inspect every request and response, easily browse JSON payloads and prompts, track token usage, and monitor backend health — all in real time.
- **Zero dependencies at runtime** — ships as a single static binary with the web console embedded. No Node.js, no Docker, no database required.

## Features

| Category | Details |
|---|---|
| **OpenAI-Compatible API** | `POST /v1/chat/completions` (streaming & non-streaming), `GET /v1/models` |
| **Backend Support** | Ollama, OpenAI, and any OpenAI-compatible provider (Azure OpenAI, Groq, Together AI, etc.) |
| **Conditional Routing** | Rule-based routing by model name, request body length, or regex match on prompt content |
| **Routing Strategies** | First available, round-robin, or pinned to a specific backend |
| **Web Console** | Configuration, dashboard with live charts, and request inspection — all in one place |
| **Dashboard** | Real-time request counts, token usage (prompt / completion), and per-backend connectivity status |
| **Request Inspection** | Full request & response JSON bodies, prompt analysis, model info, backend info, latency, and status for every proxied call |
| **Internationalization** | Web console supports English and Simplified Chinese, auto-detected from browser locale |
| **Single Binary** | Static assets compiled in via `rust-embed` — one binary, nothing else to deploy |

## Quick Start

### Install

**From GitHub Releases** (recommended)

Download a pre-built binary for your platform from the [Releases](https://github.com/mikewei/pomfret/releases) page. Available for:

- macOS (Apple Silicon & Intel)
- Linux (aarch64 & x86_64)
- Windows (x86_64)

Or use the shell installer:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/mikewei/pomfret/releases/latest/download/pomfret-installer.sh | sh
```

**Build from source**

```bash
git clone https://github.com/mikewei/pomfret.git
cd pomfret
cargo build --release
# Binary is at target/release/pomfret
```

### Run

```bash
pomfret
```

By default Pomfret listens on `127.0.0.1:8080`. Open the web console at `http://localhost:8080/console`.

### Point your client at Pomfret

Use `http://localhost:8080/v1` as the base URL in your OpenAI SDK, agent framework, or any compatible client:

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

**Using with OpenClaw** — add Pomfret as a provider in your `openclaw.conf` under `models.providers`:

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

The web console also provides a one-click OpenClaw config generator — just click the **OpenClaw Config** button on the Configuration page.

## Configuration

Pomfret can be configured via CLI flags or a TOML config file (`~/.pomfret/backends.conf` by default).

| Flag | Short | Description | Default |
|---|---|---|---|
| `--config` | `-c` | Path to backends config file | `~/.pomfret/backends.conf` |
| `--port` | `-p` | Port to listen on | `8080` |
| `--bind` | `-b` | Bind address | `127.0.0.1` |

All backend and routing configuration can be managed directly from the **web console** — add, edit, or remove LLM backends, and set up condition-based routing rules (by model name, prompt length, or regex), all without restarting the service.

## Tech Stack

- **Backend**: Rust + [Axum](https://github.com/tokio-rs/axum) — async, zero-cost abstractions, single-threaded-safe concurrency
- **HTTP Client**: [reqwest](https://github.com/seanmonstar/reqwest) with rustls — streaming support for SSE
- **Frontend**: Vanilla JavaScript + CSS — no framework, no build step
- **Packaging**: [rust-embed](https://github.com/pyrossh/rust-embed) compiles the web console into the binary
- **Distribution**: [cargo-dist](https://github.com/axodotdev/cargo-dist) for cross-platform release builds

## Development

```bash
# Run in development
cargo run

# Run tests
cargo test

# Build optimized release
cargo build --release
```

Set `RUST_LOG=pomfret=debug` for verbose logging.

## License

Pomfret is open-source software licensed under the [MIT License](LICENSE).
