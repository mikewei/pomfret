//! Console API: list requests, get one request, list backends, backend status, config save/export, routing, long-poll notify.

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use crate::config::{write_config_to_path, BackendConfig, BackendType, Config};
use crate::routing::{save_routing_config, RoutingConfig};
use crate::store::RequestSearchResult;
use crate::web::WebState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

/// Event type for long-poll notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotifyEvent {
    Requests,
    Backends,
}

impl NotifyEvent {
    fn as_str(self) -> &'static str {
        match self {
            NotifyEvent::Requests => "requests",
            NotifyEvent::Backends => "backends",
        }
    }
}

#[derive(Serialize)]
pub struct RequestListItem {
    id: String,
    method: String,
    path: String,
    backend_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend_name: Option<String>,
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend_model: Option<String>,
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_label: Option<String>,
    created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_body_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_body_size: Option<usize>,
}

#[derive(Serialize)]
pub struct BackendListItem {
    id: String,
    name: String,
    base_url: String,
    pub backend_type: BackendType,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key_set: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

#[derive(Serialize)]
pub struct BackendStatusItem {
    id: String,
    name: String,
    base_url: String,
    reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    request_count: usize,
    last_request_at: Option<f64>,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Deserialize)]
struct ConfigSaveBody {
    action: String, // "overwrite" | "save_as" (save_as handled by export endpoint)
}

#[derive(Deserialize)]
struct NotifyQuery {
    #[serde(default = "default_notify_timeout")]
    timeout: u64,
}

fn default_notify_timeout() -> u64 {
    30
}

pub fn router(state: WebState) -> Router<WebState> {
    Router::new()
        .route("/requests", get(list_requests))
        .route("/requests/search", get(search_requests))
        .route("/requests/:id", get(get_request))
        .route("/stats", get(stats))
        .route("/stats/timeseries", get(timeseries))
        .route("/notify", get(notify))
        .route("/backends", get(list_backends).post(create_backend))
        .route("/backends/status", get(backends_status))
        .route("/backends/:index", put(update_backend).delete(delete_backend))
        .route("/config/save", axum::routing::post(config_save))
        .route("/config/export", get(config_export))
        .route("/routing", get(get_routing).put(update_routing))
        .route("/routing/export", get(routing_export))
        .route("/version", get(version))
        .with_state(state)
}

/// Long-poll: wait for first notify event or timeout, drain remaining, return events.
async fn notify(
    State(state): State<WebState>,
    Query(q): Query<NotifyQuery>,
) -> impl IntoResponse {
    let timeout_secs = q.timeout.min(60);
    let mut rx = state.notify_tx.subscribe();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    let mut events: HashSet<NotifyEvent> = HashSet::new();

    loop {
        let timeout_fut = tokio::time::sleep_until(deadline);
        tokio::select! {
            res = rx.recv() => {
                match res {
                    Ok(ev) => {
                        events.insert(ev);
                        break;
                    }
                    Err(_) => break,
                }
            }
            _ = timeout_fut => break,
        }
    }

    while let Ok(ev) = rx.try_recv() {
        events.insert(ev);
    }

    let event_names: Vec<&str> = events.iter().map(|e| e.as_str()).collect();
    (
        [(header::CACHE_CONTROL, "no-cache")],
        Json(serde_json::json!({ "events": event_names })),
    )
        .into_response()
}

#[derive(Deserialize)]
struct StatsQuery {
    since: Option<f64>,
}

async fn stats(
    State(state): State<WebState>,
    Query(q): Query<StatsQuery>,
) -> Json<serde_json::Value> {
    let s = state.store.get_stats(q.since).await;
    Json(serde_json::json!({
        "total_requests": s.total,
        "total_prompt_tokens": s.total_prompt_tokens,
        "total_completion_tokens": s.total_completion_tokens,
        "total_tokens": s.total_tokens,
    }))
}

#[derive(Deserialize)]
struct TimeseriesQuery {
    #[serde(default = "default_ts_hours")]
    hours: u64,
    #[serde(default = "default_ts_bucket")]
    bucket: u64,
}
fn default_ts_hours() -> u64 { 24 }
fn default_ts_bucket() -> u64 { 60 }

async fn timeseries(
    State(state): State<WebState>,
    Query(q): Query<TimeseriesQuery>,
) -> Json<Vec<crate::store::TimeseriesBucket>> {
    let hours = q.hours.min(168);
    let bucket = q.bucket.max(10).min(3600);
    let data = state.store.get_timeseries(hours, bucket).await;
    Json(data)
}

async fn list_requests(State(state): State<WebState>) -> Json<Vec<RequestListItem>> {
    let records = state.store.list(100).await;
    let items = records
        .into_iter()
        .map(|r| RequestListItem {
            id: r.id,
            method: r.method,
            path: r.path,
            backend_id: r.backend_id,
            backend_name: r.backend_name,
            model: r.model,
            backend_model: r.backend_model,
            status: r.status,
            status_label: r.status_label,
            created_at: r.created_at,
            request_body_size: r.request_body_size,
            response_body_size: r.response_body_size,
        })
        .collect();
    Json(items)
}

const MAX_SEARCH_QUERY_LEN: usize = 256;

#[derive(Deserialize)]
struct RequestSearchQuery {
    q: String,
    #[serde(default = "default_search_limit")]
    limit: usize,
}

fn default_search_limit() -> usize {
    200
}

async fn search_requests(
    State(state): State<WebState>,
    Query(q): Query<RequestSearchQuery>,
) -> impl IntoResponse {
    let trimmed = q.q.trim();
    if trimmed.len() > MAX_SEARCH_QUERY_LEN {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "query too long" })),
        )
            .into_response();
    }
    let limit = q.limit.clamp(1, 500);
    let res: RequestSearchResult = state.store.search_request_ids(trimmed, limit).await;
    Json(res).into_response()
}

async fn get_request(
    State(state): State<WebState>,
    Path(id): Path<String>,
) -> Json<Option<serde_json::Value>> {
    let record = state.store.get(&id).await;
    Json(record.map(|r| serde_json::json!({
        "id": r.id,
        "method": r.method,
        "path": r.path,
        "request_query": r.request_query,
        "request_headers": r.request_headers,
        "backend_id": r.backend_id,
        "backend_name": r.backend_name,
        "model": r.model,
        "backend_model": r.backend_model,
        "request_body": r.request_body,
        "response_body": r.response_body,
        "status": r.status,
        "status_label": r.status_label,
        "response_headers": r.response_headers,
        "created_at": r.created_at,
        "request_body_size": r.request_body_size,
        "response_body_size": r.response_body_size,
    })))
}

async fn list_backends(State(state): State<WebState>) -> Json<Vec<BackendListItem>> {
    let backends = state.app_state.list_backends().await;
    let items = backends
        .into_iter()
        .map(|b| BackendListItem {
            api_key_set: Some(b.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false)),
            api_key_hint: b
                .api_key
                .as_ref()
                .and_then(|k| masked_secret_hint(k)),
            model: b.model.clone(),
            id: b.id,
            name: b.name,
            base_url: b.base_url,
            backend_type: b.backend_type.clone(),
        })
        .collect();
    Json(items)
}

fn masked_secret_hint(secret: &str) -> Option<String> {
    let s = secret.trim();
    if s.is_empty() {
        return None;
    }
    // Show only first/last char; keep masked length identical to plaintext length.
    let chars: Vec<char> = s.chars().collect();
    match chars.len() {
        0 => None,
        1 => Some(chars[0].to_string()),
        2 => Some(format!("{}{}", chars[0], chars[1])),
        n => {
            let mut out = String::with_capacity(s.len());
            out.push(chars[0]);
            out.extend(std::iter::repeat('*').take(n - 2));
            out.push(chars[n - 1]);
            Some(out)
        }
    }
}

/// Probe backend: GET base_url/models with short timeout.
async fn probe_backend(b: &BackendConfig) -> (bool, Option<String>) {
    // Passthrough backends may not expose a /models endpoint (e.g. Anthropic,
    // custom APIs), so skip the probe and assume reachable.
    if b.backend_type == BackendType::Passthrough {
        return (true, None);
    }
    let base = match b.backend_type {
        BackendType::Gemini => crate::providers::gemini::resolve_base_url(&b.base_url),
        _ => b.base_url.trim_end_matches('/').to_string(),
    };
    let url = format!("{}/models", base);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => return (false, Some(e.to_string())),
    };
    let mut req = client.get(&url);
    if let Some(key) = &b.api_key {
        if !key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => (true, None),
        Ok(resp) => (false, Some(format!("HTTP {}", resp.status()))),
        Err(e) => (false, Some(e.to_string())),
    }
}

async fn backends_status(State(state): State<WebState>) -> Json<Vec<BackendStatusItem>> {
    let backends = state.app_state.list_backends().await;
    let stats = state.store.get_stats(None).await;
    let mut items = Vec::with_capacity(backends.len());
    for b in backends.into_iter() {
        let (reachable, last_error) = probe_backend(&b).await;
        let bs = stats.by_backend.get(&b.id).cloned().unwrap_or_default();
        items.push(BackendStatusItem {
            id: b.id,
            name: b.name,
            base_url: b.base_url,
            reachable,
            last_error: if reachable { None } else { last_error },
            request_count: bs.count,
            last_request_at: if bs.last_at > 0.0 {
                Some(bs.last_at)
            } else {
                None
            },
            prompt_tokens: bs.prompt_tokens,
            completion_tokens: bs.completion_tokens,
            total_tokens: bs.total_tokens,
        });
    }
    Json(items)
}

#[derive(Deserialize)]
struct CreateBackendBody {
    name: String,
    base_url: String,
    api_key: Option<String>,
    #[serde(default)]
    backend_type: Option<BackendType>,
    #[serde(default)]
    model: Option<String>,
}

async fn create_backend(
    State(state): State<WebState>,
    Json(body): Json<CreateBackendBody>,
) -> Json<serde_json::Value> {
    let name = body.name.trim().to_string();
    let base_url = body.base_url.trim().to_string();
    if name.is_empty() || base_url.is_empty() {
        return Json(serde_json::json!({ "ok": false, "error": "name and base_url required" }));
    }
    let model = body.model.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let ok = state
        .app_state
        .add_backend(
            name,
            base_url,
            body.api_key.map(|s| s.trim().to_string()),
            body.backend_type,
            model,
        )
        .await;
    if ok {
        state.provider_pool.clear();
        let _ = state.notify_tx.send(NotifyEvent::Backends);
    }
    Json(serde_json::json!({ "ok": ok }))
}

#[derive(Deserialize)]
struct UpdateBackendBody {
    name: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    backend_type: Option<BackendType>,
    /// Sent as `""` to clear, omitted to leave unchanged.
    model: Option<String>,
}

async fn update_backend(
    State(state): State<WebState>,
    Path(index): Path<usize>,
    Json(body): Json<UpdateBackendBody>,
) -> Json<serde_json::Value> {
    let model_update = body.model.map(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    });
    let ok = state
        .app_state
        .update_backend(
            index,
            body.name,
            body.base_url,
            body.api_key,
            body.backend_type,
            model_update,
        )
        .await;
    if ok {
        state.provider_pool.clear();
        let _ = state.notify_tx.send(NotifyEvent::Backends);
    }
    Json(serde_json::json!({ "ok": ok }))
}

async fn delete_backend(
    State(state): State<WebState>,
    Path(index): Path<usize>,
) -> Json<serde_json::Value> {
    let ok = state.app_state.delete_backend(index).await;
    if ok {
        state.provider_pool.clear();
        let _ = state.notify_tx.send(NotifyEvent::Backends);
    }
    Json(serde_json::json!({ "ok": ok }))
}

async fn config_save(
    State(state): State<WebState>,
    Json(body): Json<ConfigSaveBody>,
) -> Json<serde_json::Value> {
    if body.action != "overwrite" {
        return Json(serde_json::json!({ "ok": false, "error": "invalid action" }));
    }
    let backends = state.app_state.list_backends().await;
    let mut config = Config::default_empty();
    config.backends = backends;
    match write_config_to_path(&state.backends_path, &config) {
        Ok(()) => Json(serde_json::json!({ "ok": true })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn get_routing(State(state): State<WebState>) -> Json<RoutingConfig> {
    let config = state.app_state.get_routing_config().await;
    Json(config)
}

async fn update_routing(
    State(state): State<WebState>,
    Json(body): Json<RoutingConfig>,
) -> Json<serde_json::Value> {
    // Persist first so in-memory state never diverges from disk on write failure.
    match save_routing_config(&state.routing_path, &body) {
        Ok(()) => {
            state.app_state.set_routing_config(body).await;
            Json(serde_json::json!({ "ok": true }))
        }
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn routing_export(State(state): State<WebState>) -> Response {
    let config = state.app_state.get_routing_config().await;
    match toml::to_string_pretty(&config) {
        Ok(toml) => (
            [
                (header::CONTENT_TYPE, "application/toml; charset=utf-8"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"routing.conf\"",
                ),
            ],
            toml,
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }))
}

async fn config_export(State(state): State<WebState>) -> Response {
    let backends = state.app_state.list_backends().await;
    let mut config = Config::default_empty();
    config.backends = backends;
    match crate::config::config_to_toml(&config) {
        Ok(toml) => (
            [
                (header::CONTENT_TYPE, "application/toml; charset=utf-8"),
                (
header::CONTENT_DISPOSITION,
                "attachment; filename=\"backends.conf\"",
                ),
            ],
            toml,
        )
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}
