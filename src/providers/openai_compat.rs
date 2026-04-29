//! OpenAI-compatible provider: forwards requests to any endpoint that speaks
//! the OpenAI `/v1/` API (OpenAI, Azure OpenAI, vLLM, LiteLLM, etc.).

use super::{LlmProvider, ProviderError, ProviderResponse};
use crate::config::BackendConfig;
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use tokio_stream::StreamExt;

pub struct OpenAiCompatProvider {
    client: Client,
    config: BackendConfig,
}

impl OpenAiCompatProvider {
    pub fn new(
        config: BackendConfig,
        backend_timeout_secs: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(
                backend_timeout_secs.max(1),
            ))
            .build()?;
        Ok(Self { client, config })
    }

    fn base_url(&self) -> String {
        self.config.base_url.trim_end_matches('/').to_string()
    }

    fn auth_header(&self) -> Option<String> {
        self.config
            .api_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .map(|k| format!("Bearer {}", k))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn chat_completions(
        &self,
        body: Bytes,
        stream: bool,
    ) -> Result<ProviderResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url());
        tracing::info!(url = %url, stream = stream, "sending chat completions to upstream");
        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_vec());

        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let res = req.send().await.map_err(ProviderError::Request)?;
        let status = res.status();
        tracing::info!(url = %url, status = %status, "upstream responded");

        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, body));
        }

        if stream {
            let stream = res.bytes_stream().map(|r| {
                r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });
            Ok(ProviderResponse::Stream(Box::pin(stream)))
        } else {
            let body = res.bytes().await.map_err(ProviderError::Request)?;
            Ok(ProviderResponse::Body(body))
        }
    }

    async fn get_models(&self) -> Result<Bytes, ProviderError> {
        let url = format!("{}/models", self.base_url());
        tracing::info!(url = %url, "sending models request to upstream");
        let mut req = self.client.get(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        let res = req.send().await.map_err(ProviderError::Request)?;
        let status = res.status();
        tracing::info!(url = %url, status = %status, "upstream models responded");
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, body));
        }
        res.bytes().await.map_err(ProviderError::Request)
    }
}
