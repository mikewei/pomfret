//! Ollama provider.
//!
//! Ollama natively exposes OpenAI-compatible `/v1/` endpoints, so this
//! implementation delegates to [`OpenAiCompatProvider`] today.  Having a
//! dedicated type lets us add Ollama-specific features later (e.g. model
//! pulling via `/api/pull`, health checks, generation parameters) without
//! touching the generic OpenAI-compat path.

use super::openai_compat::OpenAiCompatProvider;
use super::{LlmProvider, ProviderError, ProviderResponse};
use crate::config::BackendConfig;
use async_trait::async_trait;
use bytes::Bytes;

pub struct OllamaProvider {
    inner: OpenAiCompatProvider,
}

impl OllamaProvider {
    pub fn new(
        config: BackendConfig,
        backend_timeout_secs: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            inner: OpenAiCompatProvider::new(config, backend_timeout_secs)?,
        })
    }
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn chat_completions(
        &self,
        body: Bytes,
        stream: bool,
    ) -> Result<ProviderResponse, ProviderError> {
        self.inner.chat_completions(body, stream).await
    }

    async fn get_models(&self) -> Result<Bytes, ProviderError> {
        self.inner.get_models().await
    }
}
