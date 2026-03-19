//! OpenAI-compatible API handlers: parse request, call backend, return response.

use crate::config::BackendType;
use crate::providers::{create_provider, ProviderError, ProviderResponse};
use crate::routing::resolve_backend;
use crate::store::RequestRecord;
use crate::web::{NotifyEvent, WebState};
use super::tsig;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::stream::unfold;
use http_body_util::{BodyExt, StreamBody};
use http_body::Frame;
use std::collections::HashMap;
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

/// Reconstruct a chat completion response from accumulated SSE chunks.
/// Parses `data: {...}` lines, assembles content/reasoning deltas, and builds
/// a standard non-streaming response JSON. Falls back to raw text on failure.
///
/// Preserves all delta fields (not just content/reasoning_content), so that
/// provider-specific extras (e.g. Gemini `extra_content`) appear in the
/// reconstructed response shown in the inspection UI.
fn reconstruct_from_sse(raw: &str) -> String {
    let mut content = String::new();
    let mut reasoning_content = String::new();
    let mut extra_strings: HashMap<String, String> = HashMap::new();
    let mut extra_values: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let mut id: Option<String> = None;
    let mut model: Option<String> = None;
    let mut created: Option<u64> = None;
    let mut role: Option<String> = None;
    let mut finish_reason: Option<String> = None;
    let mut usage: Option<serde_json::Value> = None;
    let mut parsed_any = false;

    const KNOWN_DELTA_KEYS: &[&str] = &["role", "content", "reasoning_content"];

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
                    for (key, val) in delta.iter() {
                        if KNOWN_DELTA_KEYS.contains(&key.as_str()) {
                            continue;
                        }
                        if let Some(s) = val.as_str() {
                            extra_strings
                                .entry(key.clone())
                                .or_default()
                                .push_str(s);
                        } else if !val.is_null() {
                            extra_values.insert(key.clone(), val.clone());
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

    let mut message = serde_json::json!({
        "role": role.unwrap_or_else(|| "assistant".to_string()),
        "content": content,
    });
    if !reasoning_content.is_empty() {
        message["reasoning_content"] = serde_json::Value::String(reasoning_content);
    }
    if let Some(msg_obj) = message.as_object_mut() {
        for (k, v) in &extra_strings {
            msg_obj.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
        for (k, v) in &extra_values {
            msg_obj.insert(k.clone(), v.clone());
        }
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

    // --- TSIG request decode: strip <<TSIG:…>> markers before routing ---
    let tsig_result = tsig::strip_tsig_from_request(&body);
    if let Some(ref tr) = tsig_result {
        tracing::trace!(
            extracted_messages = tr.sigs.len(),
            "tsig: stripped markers from request (pre-routing)"
        );
    }
    let (routing_body_str, routing_body_len) = if let Some(ref tr) = tsig_result {
        let s = serde_json::to_string(&tr.body).unwrap_or_else(|_| req_body_str.clone());
        let l = s.len();
        (s, l)
    } else {
        (req_body_str.clone(), body.len())
    };

    let backend = match resolve_backend(
        &state.app_state,
        model.as_deref(),
        Some(&routing_body_str),
        routing_body_len,
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

    let is_gemini = backend.backend_type == BackendType::Gemini;

    // --- TSIG: for Gemini backends, write extracted signatures back ---
    let body = if let Some(tr) = tsig_result {
        let mut val = tr.body;
        if is_gemini {
            tsig::inject_tsig_to_gemini_request(&mut val, &tr.sigs);
            tracing::trace!(
                injected_messages = tr.sigs.len(),
                "tsig: wrote thought_signature back into request for gemini"
            );
        }
        if let Some(ref override_model) = backend.model {
            val["model"] = serde_json::Value::String(override_model.clone());
        }
        bytes::Bytes::from(serde_json::to_vec(&val).unwrap_or_else(|_| body.to_vec()))
    } else if let Some(ref override_model) = backend.model {
        match serde_json::from_slice::<serde_json::Value>(&body) {
            Ok(mut v) => {
                v["model"] = serde_json::Value::String(override_model.clone());
                bytes::Bytes::from(serde_json::to_vec(&v).unwrap_or_else(|_| body.to_vec()))
            }
            Err(_) => body,
        }
    } else {
        body
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
    let provider = match create_provider(backend) {
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
            // --- TSIG response encode (non-streaming) ---
            let final_bytes = if is_gemini {
                let maybe = tsig::inject_tsig_response_body(&resp_bytes);
                if let Some(ref b) = maybe {
                    tracing::trace!(
                        before_len = resp_bytes.len(),
                        after_len = b.len(),
                        "tsig: injected markers into non-streaming response body"
                    );
                }
                maybe.unwrap_or(resp_bytes)
            } else {
                resp_bytes
            };
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

            // --- TSIG response encode (streaming): for Gemini, transform
            //     each SSE chunk to inject <<TSIG:…>> markers into content
            //     deltas and strip the raw thought_signature fields. ---
            let recording_stream = unfold(
                (
                    s,
                    Vec::<u8>::new(),
                    store,
                    record_id,
                    resp_hdrs,
                    notify_tx,
                    is_gemini,
                    Vec::<u8>::new(),
                    false, // pending_flushed_to_client
                ),
                |state| async move {
                    let (
                        mut inner,
                        mut buf,
                        store,
                        rid,
                        hdrs,
                        tx,
                        is_gemini,
                        mut pending,
                        mut pending_flushed_to_client,
                    ) = state;
                    match inner.next().await {
                        Some(Ok(chunk)) => {
                            tracing::trace!(chunk = %String::from_utf8_lossy(&chunk), "raw stream chunk");
                            let output = if is_gemini {
                                let transformed = tsig::transform_sse_chunk(&chunk, &mut pending);
                                if transformed.is_empty() && !pending.is_empty() {
                                    tracing::trace!(
                                        chunk_len = chunk.len(),
                                        pending_len = pending.len(),
                                        "tsig: buffered partial sse data (waiting for newline)"
                                    );
                                } else if !transformed.is_empty()
                                    && transformed.as_ref() != chunk.as_ref()
                                {
                                    tracing::trace!(
                                        before_len = chunk.len(),
                                        after_len = transformed.len(),
                                        pending_len = pending.len(),
                                        "tsig: transformed sse chunk"
                                    );
                                }
                                buf.extend_from_slice(&transformed);
                                transformed
                            } else {
                                buf.extend_from_slice(&chunk);
                                chunk
                            };
                            Some((
                                Ok::<_, io::Error>(Frame::data(output)),
                                (
                                    inner,
                                    buf,
                                    store,
                                    rid,
                                    hdrs,
                                    tx,
                                    is_gemini,
                                    pending,
                                    pending_flushed_to_client,
                                ),
                            ))
                        }
                        Some(Err(e)) => Some((
                            Err(io::Error::new(io::ErrorKind::Other, e)),
                            (
                                inner,
                                buf,
                                store,
                                rid,
                                hdrs,
                                tx,
                                is_gemini,
                                pending,
                                pending_flushed_to_client,
                            ),
                        )),
                        None => {
                            // If we buffered partial lines (no newline boundaries), flush them once
                            // to the client before terminating the stream.
                            if is_gemini && !pending.is_empty() && !pending_flushed_to_client {
                                let flushed = tsig::transform_sse_chunk(b"", &mut pending);
                                pending_flushed_to_client = true;
                                tracing::trace!(
                                    flushed_len = flushed.len(),
                                    "tsig: flushed pending sse buffer at stream end"
                                );
                                buf.extend_from_slice(&flushed);
                                return Some((
                                    Ok::<_, io::Error>(Frame::data(flushed)),
                                    (
                                        inner,
                                        buf,
                                        store,
                                        rid,
                                        hdrs,
                                        tx,
                                        is_gemini,
                                        pending,
                                        pending_flushed_to_client,
                                    ),
                                ));
                            }
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
    let provider = match create_provider(backend) {
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
