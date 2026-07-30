//! Passthrough provider: forwards requests to the upstream service with
//! the exact HTTP method, path, headers, and body the client sent.
//!
//! This is intended for protocols that Pomfret does not natively understand
//! (e.g. Anthropic `POST /v1/messages` with `x-api-key`), as well as for
//! generic HTTP debugging.  The client is responsible for formatting the
//! request correctly; Pomfret only logs and relays.

use super::{LlmProvider, ProviderError, ProviderResponse};
use crate::config::BackendConfig;
use async_trait::async_trait;
use axum::http::{HeaderMap, StatusCode};
use bytes::Bytes;
use reqwest::Client;
use tokio_stream::StreamExt;

pub struct PassthroughProvider {
    client: Client,
    config: BackendConfig,
}

impl PassthroughProvider {
    pub fn new(
        config: BackendConfig,
        backend_timeout_secs: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(backend_timeout_secs.max(1)))
            .build()?;
        Ok(Self { client, config })
    }

    fn base_url(&self) -> String {
        self.config.base_url.trim_end_matches('/').to_string()
    }

    /// Build the upstream URL by appending the client's request path to the
    /// configured base URL.
    fn upstream_url(&self, path: &str) -> String {
        let trimmed = path.trim_start_matches('/');
        format!("{}/{}", self.base_url(), trimmed)
    }

    /// Convert the client's header map into a reqwest request, skipping
    /// hop-by-hop and connection-specific headers that reqwest manages.
    fn forward_headers(&self, req: reqwest::RequestBuilder, headers: &HeaderMap) -> reqwest::RequestBuilder {
        let skip: &[&str] = &[
            "host",
            "content-length",
            "connection",
            "transfer-encoding",
            "te",
            "trailer",
            "upgrade",
            "keep-alive",
        ];
        let mut req = req;
        for (key, value) in headers.iter() {
            let lower = key.as_str().to_lowercase();
            if skip.contains(&lower.as_str()) {
                continue;
            }
            // reqwest already sets its own User-Agent; forward the client's too.
            req = req.header(key.as_str(), value.as_bytes());
        }
        // The backend's stored api_key takes precedence over forwarded
        // Authorization.  If none is configured, the client's own auth header
        // (e.g. x-api-key for Anthropic) passes through above.
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        req
    }

    fn auth_header(&self) -> Option<String> {
        self.config
            .api_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .map(|k| format!("Bearer {}", k))
    }

    /// Core passthrough: send `method` + `path` + `headers` + `body` upstream
    /// and return the raw response (body or SSE stream).
    pub(crate) async fn proxy_request_impl(
        &self,
        method: &str,
        path: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<ProviderResponse, ProviderError> {
        let url = self.upstream_url(path);
        let http_method = reqwest::Method::from_bytes(method.as_bytes())
            .unwrap_or(reqwest::Method::POST);

        tracing::info!(method = %method, url = %url, "passthrough forwarding to upstream");
        let req = self.client.request(http_method, &url);
        let req = self.forward_headers(req, headers);
        let req = req.body(body.to_vec());

        let res = req.send().await.map_err(ProviderError::Request)?;
        let status = res.status();
        tracing::info!(url = %url, status = %status, "upstream passthrough responded");

        // Forward EVERY response to the caller — even non-2xx — so the
        // agent sees the real upstream body and status code.
        let status_code = axum::http::StatusCode::from_u16(status.as_u16())
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);

        let is_sse = res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|ct| ct.starts_with("text/event-stream"))
            .unwrap_or(false);

        if is_sse {
            let stream = res.bytes_stream().map(|r| {
                r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });
            Ok(ProviderResponse::Stream(Box::pin(stream)))
        } else {
            let body = res.bytes().await.map_err(ProviderError::Request)?;
            Ok(ProviderResponse::Body { bytes: body, status: status_code })
        }
    }
}

#[async_trait]
impl LlmProvider for PassthroughProvider {
    /// Forward chat completions using passthrough semantics.
    async fn chat_completions(
        &self,
        body: Bytes,
        stream: bool,
    ) -> Result<ProviderResponse, ProviderError> {
        // When called via the registered /v1/chat/completions route, behave
        // like a regular OpenAI-compatible forwarder.
        let url = format!("{}/chat/completions", self.base_url());
        tracing::info!(url = %url, stream = stream, "passthrough chat completions");
        let mut req = self.client.post(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }

        let res = req
            .header("Content-Type", "application/json")
            .body(body.to_vec())
            .send()
            .await
            .map_err(ProviderError::Request)?;
        let status = res.status();
        tracing::info!(url = %url, status = %status, "passthrough chat completions responded");

        if !status.is_success() {
            let err_body = res.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, err_body));
        }

        if stream {
            let s = res.bytes_stream().map(|r| {
                r.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });
            Ok(ProviderResponse::Stream(Box::pin(s)))
        } else {
            let b = res.bytes().await.map_err(ProviderError::Request)?;
            Ok(ProviderResponse::Body { bytes: b, status: StatusCode::OK })
        }
    }

    /// Forward model listing.
    async fn get_models(&self) -> Result<Bytes, ProviderError> {
        let url = format!("{}/models", self.base_url());
        tracing::info!(url = %url, "passthrough get_models");
        let mut req = self.client.get(&url);
        if let Some(auth) = self.auth_header() {
            req = req.header("Authorization", auth);
        }
        let res = req.send().await.map_err(ProviderError::Request)?;
        let status = res.status();
        tracing::info!(url = %url, status = %status, "passthrough get_models responded");
        if !status.is_success() {
            let err_body = res.text().await.unwrap_or_default();
            return Err(ProviderError::Status(status, err_body));
        }
        res.bytes().await.map_err(ProviderError::Request)
    }

    /// Override the default proxy_request: forward the exact method, path,
    /// and headers the client sent (true passthrough semantics).
    async fn proxy_request(
        &self,
        method: &str,
        path: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<ProviderResponse, ProviderError> {
        self.proxy_request_impl(method, path, headers, body).await
    }
}
