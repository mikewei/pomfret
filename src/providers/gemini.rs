//! Gemini provider.
//!
//! Google Gemini exposes an OpenAI-compatible endpoint at
//! `https://generativelanguage.googleapis.com/v1beta/openai`, so this
//! implementation delegates to [`OpenAiCompatProvider`].  The main added
//! value is automatic base-URL resolution: users can supply just the
//! domain or a partial path and the provider fills in the rest.

use super::openai_compat::OpenAiCompatProvider;
use super::{LlmProvider, ProviderError, ProviderResponse};
use crate::config::BackendConfig;
use async_trait::async_trait;
use bytes::Bytes;

const GEMINI_DEFAULT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/openai";

/// Resolve a user-supplied base URL to the full Gemini OpenAI-compat path.
pub fn resolve_base_url(raw: &str) -> String {
    let trimmed = raw.trim_end_matches('/');
    if trimmed.is_empty() {
        return GEMINI_DEFAULT_BASE.to_string();
    }
    if trimmed.ends_with("/openai") {
        return trimmed.to_string();
    }
    if trimmed.ends_with("/v1beta") {
        return format!("{}/openai", trimmed);
    }
    format!("{}/v1beta/openai", trimmed)
}

pub struct GeminiProvider {
    inner: OpenAiCompatProvider,
}

impl GeminiProvider {
    pub fn new(mut config: BackendConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        config.base_url = resolve_base_url(&config.base_url);
        Ok(Self {
            inner: OpenAiCompatProvider::new(config)?,
        })
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_empty_url() {
        assert_eq!(resolve_base_url(""), GEMINI_DEFAULT_BASE);
    }

    #[test]
    fn resolve_domain_only() {
        assert_eq!(
            resolve_base_url("https://generativelanguage.googleapis.com"),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
    }

    #[test]
    fn resolve_domain_with_trailing_slash() {
        assert_eq!(
            resolve_base_url("https://generativelanguage.googleapis.com/"),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
    }

    #[test]
    fn resolve_v1beta_path() {
        assert_eq!(
            resolve_base_url("https://generativelanguage.googleapis.com/v1beta"),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
    }

    #[test]
    fn resolve_full_path_unchanged() {
        assert_eq!(
            resolve_base_url("https://generativelanguage.googleapis.com/v1beta/openai"),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
    }

    #[test]
    fn resolve_full_path_with_trailing_slash() {
        assert_eq!(
            resolve_base_url("https://generativelanguage.googleapis.com/v1beta/openai/"),
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
    }
}
