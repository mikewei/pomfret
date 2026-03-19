//! Console API tests: /api/requests, /api/backends.

use axum::body::Body;
use axum::http::Request;
use pomfret::config::{AppState, Config};
use pomfret::store::{MemoryStore, RequestRecord};
use pomfret::web::{router, NotifyEvent, ProviderPool, WebState};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tower::ServiceExt;

fn make_state() -> WebState {
    let config = Config::default_with_examples();
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let (notify_tx, _) = broadcast::channel::<NotifyEvent>(32);
    WebState {
        app_state,
        store,
        backends_path: PathBuf::from("/tmp/pomfret-test-backends.conf"),
        routing_path: PathBuf::from("/tmp/pomfret-test-routing.conf"),
        notify_tx,
        provider_pool: ProviderPool::new(),
    }
}

#[tokio::test]
async fn api_backends_list_and_add() {
    let app = router(make_state());

    let req = Request::builder().uri("/api/backends").body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert!(res.status().is_success());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(list.len() >= 1);
    assert!(list[0]["id"].as_str().map_or(false, |s| !s.is_empty()));
    assert_eq!(list[0]["name"], "Ollama");
    assert_eq!(list[0]["backend_type"], "ollama");

    let req = Request::builder()
        .method("POST")
        .uri("/api/backends")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name":"OpenAI","base_url":"https://api.openai.com","backend_type":"openai_compat"}"#,
        ))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert!(res.status().is_success());

    let req = Request::builder().uri("/api/backends").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[1]["name"], "OpenAI");
}

#[tokio::test]
async fn api_requests_list_and_get() {
    let state = make_state();
    let record = RequestRecord::new(
        "POST".to_string(),
        "/v1/chat/completions".to_string(),
        None,
        None,
        Some("ollama".to_string()),
        None,
        Some("llama2".to_string()),
        None,
        Some(r#"{"messages":[]}"#.to_string()),
    );
    let id = record.id.clone();
    state.store.push(record).await;

    let app = router(state);

    let req = Request::builder().uri("/api/requests").body(Body::empty()).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert!(res.status().is_success());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(!list.is_empty());
    assert_eq!(list[0]["id"], id);

    let req = Request::builder()
        .uri(format!("/api/requests/{}", id))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert!(res.status().is_success());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let one: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(one["id"], id);
    assert_eq!(one["request_body"], r#"{"messages":[]}"#);
}
