//! Gemini provider.
//!
//! Google Gemini exposes an OpenAI-compatible endpoint at
//! `https://generativelanguage.googleapis.com/v1beta/openai`, so this
//! implementation delegates to [`OpenAiCompatProvider`].  The main added
//! value is automatic base-URL resolution and server-side
//! `thought_signature` cache (strip from responses, inject on requests).
//! Message-level signatures embed `SIGID:<id>` inside `<think>...</think>` in `content` (server-generated ids).

mod thought_signature;

use super::openai_compat::OpenAiCompatProvider;
use super::{LlmProvider, ProviderError, ProviderResponse};
use crate::config::BackendConfig;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::unfold;
use std::io;
use std::pin::Pin;
use thought_signature::ThoughtSignatureCache;
use tokio_stream::StreamExt;

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
    thought_signatures: ThoughtSignatureCache,
}

impl GeminiProvider {
    pub fn new(
        mut config: BackendConfig,
        backend_timeout_secs: u64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        config.base_url = resolve_base_url(&config.base_url);
        Ok(Self {
            inner: OpenAiCompatProvider::new(config, backend_timeout_secs)?,
            thought_signatures: ThoughtSignatureCache::new(),
        })
    }

    fn prepare_request_body(&self, body: Bytes) -> Bytes {
        match serde_json::from_slice::<serde_json::Value>(&body) {
            Ok(mut v) => {
                // Gemini OpenAI-compat rejects unknown top-level fields like `store`.
                if let Some(obj) = v.as_object_mut() {
                    obj.remove("store");
                }
                thought_signature::inject_cached_signatures_into_gemini_request(
                    &mut v,
                    &self.thought_signatures,
                );
                serde_json::to_vec(&v)
                    .map(Bytes::from)
                    .unwrap_or_else(|_| body)
            }
            Err(_) => body,
        }
    }

    fn process_response_body(&self, resp_bytes: Bytes) -> Bytes {
        match serde_json::from_slice::<serde_json::Value>(&resp_bytes) {
            Ok(mut v) => {
                if thought_signature::cache_signatures_from_response_value(&mut v, &self.thought_signatures)
                {
                    tracing::trace!(
                        before_len = resp_bytes.len(),
                        "tsig: cached signatures and stripped from non-streaming body"
                    );
                    serde_json::to_vec(&v)
                        .map(Bytes::from)
                        .unwrap_or_else(|_| resp_bytes)
                } else {
                    resp_bytes
                }
            }
            Err(_) => resp_bytes,
        }
    }

    fn wrap_sse_stream(
        s: Pin<Box<dyn futures_util::Stream<Item = Result<Bytes, io::Error>> + Send>>,
        cache: ThoughtSignatureCache,
    ) -> Pin<Box<dyn futures_util::Stream<Item = Result<Bytes, io::Error>> + Send>> {
        let stream = unfold(
            (s, Vec::<u8>::new(), cache, false),
            |(mut inner, mut pending, cache, pending_flushed)| async move {
                match inner.next().await {
                    Some(Ok(chunk)) => {
                        let transformed = thought_signature::transform_sse_chunk_cache_strip(
                            &chunk,
                            &mut pending,
                            &cache,
                        );
                        if transformed.is_empty() && !pending.is_empty() {
                            tracing::trace!(
                                chunk_len = chunk.len(),
                                pending_len = pending.len(),
                                "tsig: buffered partial sse data (waiting for newline)"
                            );
                        } else if !transformed.is_empty() && transformed.as_ref() != chunk.as_ref() {
                            tracing::trace!(
                                before_len = chunk.len(),
                                after_len = transformed.len(),
                                pending_len = pending.len(),
                                "tsig: transformed sse chunk"
                            );
                        }
                        Some((
                            Ok(transformed),
                            (inner, pending, cache, pending_flushed),
                        ))
                    }
                    Some(Err(e)) => Some((Err(e), (inner, pending, cache, pending_flushed))),
                    None => {
                        if !pending.is_empty() && !pending_flushed {
                            let flushed = thought_signature::transform_sse_chunk_cache_strip(
                                b"",
                                &mut pending,
                                &cache,
                            );
                            tracing::trace!(
                                flushed_len = flushed.len(),
                                "tsig: flushed pending sse buffer at stream end"
                            );
                            return Some((
                                Ok(flushed),
                                (inner, pending, cache, true),
                            ));
                        }
                        None
                    }
                }
            },
        );
        Box::pin(stream)
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn chat_completions(
        &self,
        body: Bytes,
        stream: bool,
    ) -> Result<ProviderResponse, ProviderError> {
        let body = self.prepare_request_body(body);
        let res = self.inner.chat_completions(body, stream).await?;
        match res {
            ProviderResponse::Body(resp_bytes) => {
                Ok(ProviderResponse::Body(self.process_response_body(resp_bytes)))
            }
            ProviderResponse::Stream(s) => {
                let cache = self.thought_signatures.clone();
                Ok(ProviderResponse::Stream(Self::wrap_sse_stream(s, cache)))
            }
        }
    }

    async fn get_models(&self) -> Result<Bytes, ProviderError> {
        self.inner.get_models().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BackendConfig, BackendType};

    #[test]
    fn prepare_request_body_strips_store() {
        let config = BackendConfig {
            id: "t".into(),
            name: "t".into(),
            base_url: "".into(),
            api_key: None,
            backend_type: BackendType::Gemini,
            model: None,
        };
        let p = GeminiProvider::new(config, 120).unwrap();
        let body = r#"{"model":"x","store":true,"messages":[]}"#;
        let out = p.prepare_request_body(Bytes::from(body));
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v.get("store").is_none(), "Gemini must not forward `store`");
        assert_eq!(v.get("model").and_then(|m| m.as_str()), Some("x"));
        assert!(v.get("messages").is_some());
    }

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
