//! OpenAI-compatible API handlers: parse request, call backend, return response.

use crate::providers::{ProviderError, ProviderResponse};
use crate::routing::resolve_backend;
use crate::store::RequestRecord;
use crate::web::{NotifyEvent, WebState};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::stream::unfold;
use http_body_util::{BodyExt, StreamBody};
use http_body::Frame;
use std::collections::{BTreeMap, HashMap};
use std::io;
use tokio_stream::StreamExt;

/// Minimal request shape to read stream and model for logging.
#[derive(serde::Deserialize, Default)]
struct ChatRequestMin {
    #[serde(default)]
    stream: bool,
    model: Option<String>,
}

/// Serialize request headers to JSON; redact Authorization.
fn request_headers_json(headers: &HeaderMap) -> Option<String> {
    if headers.is_empty() {
        return None;
    }
    let mut map: HashMap<String, String> = HashMap::new();
    for (k, v) in headers.iter() {
        let val = if k == header::AUTHORIZATION {
            "[REDACTED]".to_string()
        } else {
            v.to_str().unwrap_or("[invalid]").to_string()
        };
        map.insert(k.as_str().to_string(), val);
    }
    serde_json::to_string(&map).ok()
}

/// Build response headers JSON for logging.
fn response_headers_json(content_type: &str) -> String {
    let mut map = HashMap::new();
    map.insert("Content-Type".to_string(), content_type.to_string());
    serde_json::to_string(&map).unwrap_or_default()
}

/// Merge one streaming `tool_calls[]` delta entry into an accumulated tool call (by `index`).
fn merge_tool_call_object(
    acc: &mut serde_json::Map<String, serde_json::Value>,
    delta: &serde_json::Map<String, serde_json::Value>,
) {
    for (k, v) in delta {
        if k == "index" {
            continue;
        }
        if k == "function" {
            let func_acc = acc
                .entry("function".to_string())
                .or_insert_with(|| serde_json::json!({}));
            if let (Some(fo), Some(vo)) = (func_acc.as_object_mut(), v.as_object()) {
                for (fk, fv) in vo {
                    if fk == "arguments" {
                        let prev = fo.get("arguments").and_then(|x| x.as_str()).unwrap_or("");
                        if let Some(part) = fv.as_str() {
                            fo.insert(
                                "arguments".to_string(),
                                serde_json::Value::String(format!("{prev}{part}")),
                            );
                        } else {
                            fo.insert(fk.clone(), fv.clone());
                        }
                    } else {
                        fo.insert(fk.clone(), fv.clone());
                    }
                }
            } else {
                acc.insert(k.clone(), v.clone());
            }
        } else {
            acc.insert(k.clone(), v.clone());
        }
    }
}

/// Reconstruct a chat completion response from accumulated SSE chunks.
/// Parses `data: {...}` lines, assembles content/reasoning deltas, and builds
/// a standard non-streaming response JSON. Falls back to raw text on failure.
///
/// Merges `content`, `reasoning_content`, `role`, and streaming `tool_calls`
/// (by OpenAI-style `index`, concatenating `function.arguments`). Provider-specific
/// delta keys other than these are still ignored.
fn reconstruct_from_sse(raw: &str) -> String {
    let mut content = String::new();
    let mut reasoning_content = String::new();
    let mut tool_calls_by_index: BTreeMap<u64, serde_json::Map<String, serde_json::Value>> =
        BTreeMap::new();
    let mut id: Option<String> = None;
    let mut model: Option<String> = None;
    let mut created: Option<u64> = None;
    let mut role: Option<String> = None;
    let mut finish_reason: Option<String> = None;
    let mut usage: Option<serde_json::Value> = None;
    let mut parsed_any = false;

    for line in raw.lines() {
        let data = match line
            .strip_prefix("data: ")
            .or_else(|| line.strip_prefix("data:"))
        {
            Some(d) => d.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            continue;
        }
        let chunk: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        parsed_any = true;

        if id.is_none() {
            id = chunk.get("id").and_then(|v| v.as_str()).map(String::from);
        }
        if model.is_none() {
            model = chunk
                .get("model")
                .and_then(|v| v.as_str())
                .map(String::from);
        }
        if created.is_none() {
            created = chunk.get("created").and_then(|v| v.as_u64());
        }
        if let Some(choices) = chunk.get("choices").and_then(|v| v.as_array()) {
            for choice in choices {
                if let Some(delta) = choice.get("delta").and_then(|v| v.as_object()) {
                    if role.is_none() {
                        role = delta
                            .get("role")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                    }
                    if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                        content.push_str(c);
                    }
                    if let Some(c) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                        reasoning_content.push_str(c);
                    }
                    if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tcs {
                            let Some(tc_obj) = tc.as_object() else {
                                continue;
                            };
                            let idx = tc_obj
                                .get("index")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let entry = tool_calls_by_index.entry(idx).or_default();
                            merge_tool_call_object(entry, tc_obj);
                        }
                    }
                }
                if finish_reason.is_none() {
                    if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                        finish_reason = Some(fr.to_string());
                    }
                }
            }
        }
        if usage.is_none() {
            if let Some(u) = chunk.get("usage") {
                if !u.is_null() {
                    usage = Some(u.clone());
                }
            }
        }
    }

    if !parsed_any {
        return raw.to_string();
    }

    let has_tool_calls = !tool_calls_by_index.is_empty();
    let mut message = serde_json::json!({
        "role": role.unwrap_or_else(|| "assistant".to_string()),
        "content": if content.is_empty() && has_tool_calls {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(content)
        },
    });
    if !reasoning_content.is_empty() {
        message["reasoning_content"] = serde_json::Value::String(reasoning_content);
    }
    if !tool_calls_by_index.is_empty() {
        let tool_calls: Vec<serde_json::Value> = tool_calls_by_index
            .into_iter()
            .map(|(_, mut m)| {
                m.remove("index");
                serde_json::Value::Object(m)
            })
            .collect();
        message["tool_calls"] = serde_json::Value::Array(tool_calls);
    }

    let mut result = serde_json::json!({
        "id": id.unwrap_or_default(),
        "object": "chat.completion (reconstructed from stream)",
        "created": created.unwrap_or(0),
        "model": model.unwrap_or_default(),
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": finish_reason.unwrap_or_else(|| "stop".to_string()),
        }],
    });
    if let Some(u) = usage {
        result["usage"] = u;
    }

    serde_json::to_string(&result).unwrap_or_else(|_| raw.to_string())
}

/// Extract token usage from a response body JSON string.
/// Supports OpenAI format (prompt_tokens/completion_tokens/total_tokens)
/// and Anthropic format (input_tokens/output_tokens).
fn extract_usage(body: &str) -> (Option<u64>, Option<u64>, Option<u64>) {
    let val: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return (None, None, None),
    };
    let usage = match val.get("usage") {
        Some(u) if !u.is_null() => u,
        _ => return (None, None, None),
    };
    let prompt = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(|v| v.as_u64());
    let completion = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(|v| v.as_u64());
    let total = usage
        .get("total_tokens")
        .and_then(|v| v.as_u64())
        .or_else(|| match (prompt, completion) {
            (Some(p), Some(c)) => Some(p + c),
            _ => None,
        });
    (prompt, completion, total)
}

/// POST /v1/chat/completions — forward to current backend.
/// Returns 503 if no backend selected; 502 on backend error.
#[tracing::instrument(skip(state, request))]
pub async fn handle_chat_completions(
    State(state): State<WebState>,
    request: Request,
) -> impl IntoResponse {
    let (parts, body) = request.into_parts();
    let body = match body.collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                [("Content-Type", "application/json")],
                r#"{"error":{"message":"Failed to read body","type":"gateway_error"}}"#,
            )
                .into_response();
        }
    };
    let request_query = parts.uri.query().map(|s| s.to_string());
    let request_headers = request_headers_json(&parts.headers);

    let req_body_str = String::from_utf8_lossy(&body).to_string();
    let stream = serde_json::from_str::<ChatRequestMin>(&req_body_str)
        .map(|r| r.stream)
        .unwrap_or(false);
    let model = serde_json::from_str::<ChatRequestMin>(&req_body_str)
        .ok()
        .and_then(|r| r.model);

    let backend = match resolve_backend(
        &state.app_state,
        model.as_deref(),
        Some(&req_body_str),
        body.len(),
    )
    .await
    {
        Some(b) => b,
        None => {
            tracing::warn!("chat completions called but no backend available via routing");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("Content-Type", "application/json")],
                r#"{"error":{"message":"No backend available","type":"gateway_error"}}"#,
            )
                .into_response();
        }
    };

    // Optional model override (Gemini thought_signature inject/strip is inside GeminiProvider).
    let body = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(mut v) => {
            if let Some(ref override_model) = backend.model {
                v["model"] = serde_json::Value::String(override_model.clone());
            }
            bytes::Bytes::from(serde_json::to_vec(&v).unwrap_or_else(|_| body.to_vec()))
        }
        Err(_) => body,
    };

    tracing::info!(
        model = model.as_deref().unwrap_or("-"),
        stream = stream,
        backend = %backend.name,
        "received POST /v1/chat/completions"
    );

    let backend_id = backend.id.clone();
    let backend_name = backend.name.clone();
    let actual_model = backend.model.clone().or_else(|| model.clone());
    let provider = match state.provider_pool.get_or_create(backend) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "application/json")],
                r#"{"error":{"message":"Failed to create provider","type":"gateway_error"}}"#,
            )
                .into_response();
        }
    };

    let record = RequestRecord::new(
        "POST".to_string(),
        "/v1/chat/completions".to_string(),
        request_query,
        request_headers,
        Some(backend_id),
        Some(backend_name.clone()),
        model.clone(),
        actual_model,
        Some(req_body_str),
    );
    let record_id = record.id.clone();
    state.store.push(record).await;
    let _ = state.notify_tx.send(NotifyEvent::Requests);

    tracing::info!(backend = %backend_name, "forwarding chat completions to backend");

    match provider.chat_completions(body, stream).await {
        Ok(ProviderResponse::Body(resp_bytes)) => {
            tracing::info!(backend = %backend_name, status = 200, stream = false, "backend responded");
            tracing::trace!(backend = %backend_name, body = %String::from_utf8_lossy(&resp_bytes), "raw response body");
            let status = StatusCode::OK;
            let final_bytes = resp_bytes;
            let resp_str = String::from_utf8_lossy(&final_bytes).to_string();
            let resp_hdrs = response_headers_json("application/json");
            let (pt, ct, tt) = extract_usage(&resp_str);
            state
                .store
                .update_response(&record_id, Some(resp_str), Some(status.as_u16()), Some(resp_hdrs))
                .await;
            state.store.update_tokens(&record_id, pt, ct, tt).await;
            let _ = state.notify_tx.send(NotifyEvent::Requests);
            (
                status,
                [(header::CONTENT_TYPE, "application/json")],
                final_bytes.to_vec(),
            )
                .into_response()
        }
        Ok(ProviderResponse::Stream(s)) => {
            tracing::info!(backend = %backend_name, status = 200, stream = true, "backend responded with stream");
            let store = state.store.clone();
            let notify_tx = state.notify_tx.clone();
            let resp_hdrs = response_headers_json("text/event-stream");

            let recording_stream = unfold(
                (s, Vec::<u8>::new(), store, record_id, resp_hdrs, notify_tx),
                |state| async move {
                    let (mut inner, mut buf, store, rid, hdrs, tx) = state;
                    match inner.next().await {
                        Some(Ok(chunk)) => {
                            tracing::trace!(chunk = %String::from_utf8_lossy(&chunk), "raw stream chunk");
                            buf.extend_from_slice(&chunk);
                            Some((
                                Ok::<_, io::Error>(Frame::data(chunk)),
                                (inner, buf, store, rid, hdrs, tx),
                            ))
                        }
                        Some(Err(e)) => Some((
                            Err(io::Error::new(io::ErrorKind::Other, e)),
                            (inner, buf, store, rid, hdrs, tx),
                        )),
                        None => {
                            let raw = String::from_utf8_lossy(&buf).to_string();
                            let body = reconstruct_from_sse(&raw);
                            let (pt, ct, tt) = extract_usage(&body);
                            store
                                .update_response(
                                    &rid,
                                    Some(body),
                                    Some(200),
                                    Some(hdrs),
                                )
                                .await;
                            store.update_tokens(&rid, pt, ct, tt).await;
                            let _ = tx.send(NotifyEvent::Requests);
                            None
                        }
                    }
                },
            );

            let stream_body = StreamBody::new(recording_stream);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/event-stream")],
                Body::new(stream_body),
            )
                .into_response()
        }
        Err(e) => {
            let (code, msg) = match &e {
                ProviderError::Status(status, body) => (*status, body.clone()),
                _ => (StatusCode::BAD_GATEWAY, e.to_string()),
            };
            tracing::info!(backend = %backend_name, status = %code, "backend error");
            tracing::trace!(backend = %backend_name, status = %code, body = %msg, "raw error response body");
            let resp_hdrs = response_headers_json("application/json");
            state
                .store
                .update_response(
                    &record_id,
                    Some(msg.clone()),
                    Some(code.as_u16()),
                    Some(resp_hdrs),
                )
                .await;
            let _ = state.notify_tx.send(NotifyEvent::Requests);
            let err_json = serde_json::json!({
                "error": { "message": msg, "type": "gateway_error" }
            });
            (
                code,
                [("Content-Type", "application/json")],
                err_json.to_string(),
            )
                .into_response()
        }
    }
}

/// GET /v1/models — forward to default routing backend; return 503 if no backend.
#[tracing::instrument(skip(state, request))]
pub async fn handle_models(
    State(state): State<WebState>,
    request: Request,
) -> impl IntoResponse {
    let (parts, _) = request.into_parts();
    let request_query = parts.uri.query().map(|s| s.to_string());
    let request_headers = request_headers_json(&parts.headers);
    let backend = match resolve_backend(&state.app_state, None, None, 0).await {
        Some(b) => b,
        None => {
            tracing::warn!("GET /v1/models called but no backend available");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("Content-Type", "application/json")],
                r#"{"error":{"message":"No backend available","type":"gateway_error"}}"#,
            )
                .into_response();
        }
    };
    tracing::info!(backend = %backend.name, "received GET /v1/models");

    let backend_id = backend.id.clone();
    let backend_name = backend.name.clone();
    let provider = match state.provider_pool.get_or_create(backend) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [("Content-Type", "application/json")],
                r#"{"error":{"message":"Failed to create provider","type":"gateway_error"}}"#,
            )
                .into_response();
        }
    };

    let record = RequestRecord::new(
        "GET".to_string(),
        "/v1/models".to_string(),
        request_query,
        request_headers,
        Some(backend_id),
        Some(backend_name.clone()),
        None,
        None,
        None,
    );
    let record_id = record.id.clone();
    state.store.push(record).await;
    let _ = state.notify_tx.send(NotifyEvent::Requests);

    tracing::info!(backend = %backend_name, "forwarding models request to backend");

    let resp_hdrs_ok = response_headers_json("application/json");
    match provider.get_models().await {
        Ok(bytes) => {
            tracing::info!(backend = %backend_name, status = 200, "backend models responded");
            tracing::trace!(backend = %backend_name, body = %String::from_utf8_lossy(&bytes), "raw models response body");
            let resp_str = String::from_utf8_lossy(&bytes).to_string();
            state
                .store
                .update_response(&record_id, Some(resp_str), Some(StatusCode::OK.as_u16()), Some(resp_hdrs_ok))
                .await;
            let _ = state.notify_tx.send(NotifyEvent::Requests);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                bytes.to_vec(),
            )
                .into_response()
        }
        Err(e) => {
            let (code, msg) = match &e {
                ProviderError::Status(status, body) => (*status, body.clone()),
                _ => (StatusCode::BAD_GATEWAY, e.to_string()),
            };
            tracing::info!(backend = %backend_name, status = %code, "backend models error");
            tracing::trace!(backend = %backend_name, status = %code, body = %msg, "raw models error response body");
            state
                .store
                .update_response(&record_id, Some(msg.clone()), Some(code.as_u16()), Some(resp_hdrs_ok))
                .await;
            let _ = state.notify_tx.send(NotifyEvent::Requests);
            let err_json =
                serde_json::json!({ "error": { "message": msg, "type": "gateway_error" } });
            (code, [("Content-Type", "application/json")], err_json.to_string()).into_response()
        }
    }
}

#[cfg(test)]
mod reconstruct_tests {
    use super::*;

    #[test]
    fn reconstruct_merges_streaming_tool_calls() {
        let raw = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"tool_calls\":",
            "[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":",
            "{\"name\":\"f\",\"arguments\":\"\"}}]}}]}\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":",
            "[{\"index\":0,\"function\":{\"arguments\":\"{\\\"x\\\":1}\"}}]}}]}\n",
        );
        let s = reconstruct_from_sse(raw);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        let tc = &v["choices"][0]["message"]["tool_calls"][0];
        assert_eq!(tc["id"], "c1");
        assert_eq!(tc["function"]["name"], "f");
        assert_eq!(tc["function"]["arguments"].as_str().unwrap(), "{\"x\":1}");
        assert!(tc.get("index").is_none());
    }

    #[test]
    fn reconstruct_tool_calls_only_uses_null_content() {
        let raw = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":",
            "[{\"index\":0,\"id\":\"c1\",\"type\":\"function\",\"function\":",
            "{\"name\":\"g\",\"arguments\":\"{}\"}}]}}]}\n",
        );
        let s = reconstruct_from_sse(raw);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v["choices"][0]["message"]["content"].is_null());
        assert!(v["choices"][0]["message"]["tool_calls"].is_array());
    }
}
