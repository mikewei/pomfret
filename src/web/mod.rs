//! Web console: static assets and API routes.

mod api;

pub use api::NotifyEvent;

use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use crate::embed::EmbeddedAssets;
use crate::config::AppState;
use crate::proxy;
use crate::store::MemoryStore;
use axum::http::StatusCode;
use tokio::sync::broadcast;

/// Combined state for web routes (config + store).
#[derive(Clone)]
pub struct WebState {
    pub app_state: AppState,
    pub store: MemoryStore,
    /// Backends config file path (default ~/.pomfret/backends.conf or -c); used when saving.
    pub backends_path: std::path::PathBuf,
    /// Sender for long-poll notifications (requests/backends changed).
    pub notify_tx: broadcast::Sender<NotifyEvent>,
}

pub fn router(state: WebState) -> Router {
    let api_router = api::router(state.clone());
    Router::new()
        .route("/health", get(health))
        .route("/v1/chat/completions", axum::routing::post(proxy::handle_chat_completions))
        .route("/v1/models", axum::routing::get(proxy::handle_models))
        .nest("/api", api_router)
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/console", get(serve_index))
        .route("/console/*path", get(serve_console_fallback))
        .route("/static/*path", get(serve_static))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn serve_index() -> Response {
    match EmbeddedAssets::get("index.html") {
        Some(content) => ([("Content-Type", "text/html")], content.data.to_vec()).into_response(),
        None => (StatusCode::NOT_FOUND, "console not built").into_response(),
    }
}

async fn serve_console_fallback(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    let _ = path;
    serve_index().await
}

async fn serve_static(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    // rust_embed stores paths relative to folder, so key is e.g. "css/main.css" not "static/css/main.css"
    let key = path.trim_start_matches('/');
    match EmbeddedAssets::get(key) {
        Some(content) => {
            let mime = mime_guess::from_path(key).first_or_octet_stream();
            ([("Content-Type", mime.as_ref())], content.data.to_vec()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
