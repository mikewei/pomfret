//! Provider abstraction: pluggable LLM backend implementations.
//!
//! Each provider implements [`LlmProvider`] to handle chat completions and
//! model listing. Use [`create_provider`] to instantiate the right
//! implementation based on [`BackendType`].

pub(crate) mod gemini;
mod ollama;
mod openai_compat;
pub(crate) mod passthrough;

use crate::config::{BackendConfig, BackendType};
use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use futures_util::Stream;
use std::pin::Pin;

/// Errors from provider calls.
#[derive(Debug)]
pub enum ProviderError {
    Request(reqwest::Error),
    Status(reqwest::StatusCode, String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Request(e) => write!(f, "request error: {}", e),
            ProviderError::Status(code, body) => write!(f, "status {}: {}", code, body),
        }
    }
}

impl std::error::Error for ProviderError {}

/// Response from a provider: either a complete body with status code, or a byte stream.
pub enum ProviderResponse {
    Body {
        bytes: Bytes,
        status: axum::http::StatusCode,
    },
    Stream(Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>),
}

/// Trait for LLM providers.
///
/// Implementations translate incoming OpenAI-compatible requests into
/// whatever protocol the upstream service expects.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Forward a chat completion request. `body` is the raw JSON from the
    /// client; `stream` indicates whether the caller wants SSE streaming.
    async fn chat_completions(
        &self,
        body: Bytes,
        stream: bool,
    ) -> Result<ProviderResponse, ProviderError>;

    /// List available models from the upstream service.
    async fn get_models(&self) -> Result<Bytes, ProviderError>;

    /// Proxy an arbitrary HTTP request to the upstream.
    ///
    /// The default implementation parses `stream` from a JSON body and
    /// delegates to [`chat_completions`](LlmProvider::chat_completions).
    /// Passthrough providers override this to forward the exact method,
    /// path, and headers the client sent.
    async fn proxy_request(
        &self,
        _method: &str,
        _path: &str,
        _headers: &HeaderMap,
        body: Bytes,
    ) -> Result<ProviderResponse, ProviderError> {
        let stream = String::from_utf8_lossy(&body)
            .parse::<serde_json::Value>()
            .ok()
            .and_then(|v| v.get("stream")?.as_bool())
            .unwrap_or(false);
        self.chat_completions(body, stream).await
    }
}

/// Create a provider instance from backend configuration.
pub fn create_provider(
    config: BackendConfig,
    backend_timeout_secs: u64,
) -> Result<Box<dyn LlmProvider>, Box<dyn std::error::Error + Send + Sync>> {
    match config.backend_type {
        BackendType::OpenAiCompat => Ok(Box::new(openai_compat::OpenAiCompatProvider::new(
            config,
            backend_timeout_secs,
        )?)),
        BackendType::Ollama => Ok(Box::new(ollama::OllamaProvider::new(
            config,
            backend_timeout_secs,
        )?)),
        BackendType::Gemini => Ok(Box::new(gemini::GeminiProvider::new(
            config,
            backend_timeout_secs,
        )?)),
        BackendType::Passthrough => Ok(Box::new(passthrough::PassthroughProvider::new(
            config,
            backend_timeout_secs,
        )?)),
    }
}
