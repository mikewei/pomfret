//! Provider abstraction: pluggable LLM backend implementations.
//!
//! Each provider implements [`LlmProvider`] to handle chat completions and
//! model listing. Use [`create_provider`] to instantiate the right
//! implementation based on [`BackendType`].

pub(crate) mod gemini;
mod ollama;
mod openai_compat;

use crate::config::{BackendConfig, BackendType};
use async_trait::async_trait;
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

/// Response from a provider: either a complete body or a byte stream.
pub enum ProviderResponse {
    Body(Bytes),
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
}

/// Create a provider instance from backend configuration.
pub fn create_provider(
    config: BackendConfig,
) -> Result<Box<dyn LlmProvider>, Box<dyn std::error::Error + Send + Sync>> {
    match config.backend_type {
        BackendType::OpenAiCompat => {
            Ok(Box::new(openai_compat::OpenAiCompatProvider::new(config)?))
        }
        BackendType::Ollama => Ok(Box::new(ollama::OllamaProvider::new(config)?)),
        BackendType::Gemini => Ok(Box::new(gemini::GeminiProvider::new(config)?)),
    }
}
