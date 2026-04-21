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

#[tokio::test]
async fn api_requests_search_order_count_truncation() {
    let state = make_state();

    let r1 = RequestRecord::new(
        "POST".to_string(),
        "/a".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        Some("no match".to_string()),
    );
    state.store.push(r1).await;

    let r2 = RequestRecord::new(
        "POST".to_string(),
        "/b".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        Some(r#"{"NEEDLE":1}"#.to_string()),
    );
    let id2 = r2.id.clone();
    state.store.push(r2).await;

    let r3 = RequestRecord::new(
        "POST".to_string(),
        "/c".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        Some("{}".to_string()),
    );
    let id3 = r3.id.clone();
    state.store.push(r3).await;
    state
        .store
        .update_response(
            &id3,
            Some(r#"{"msg":"needle"}"#.to_string()),
            Some(200),
            None,
        )
        .await;

    let app = router(state);

    let req = Request::builder()
        .uri("/api/requests/search?q=needle&limit=10")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert!(res.status().is_success());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["matched_records"], 2);
    assert_eq!(v["truncated"], false);
    let ids: Vec<String> = v["ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![id3, id2]);

    let req = Request::builder()
        .uri(format!(
            "/api/requests/search?q={}&limit=2",
            "x".repeat(300)
        ))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::BAD_REQUEST);

    // Truncation: four matches, limit 2
    let state2 = make_state();
    for i in 0..4 {
        let r = RequestRecord::new(
            "GET".to_string(),
            format!("/t{i}"),
            None,
            None,
            None,
            None,
            None,
            None,
            Some("findme-unique".to_string()),
        );
        state2.store.push(r).await;
    }
    let app2 = router(state2);
    let req = Request::builder()
        .uri("/api/requests/search?q=findme&limit=2")
        .body(Body::empty())
        .unwrap();
    let res = app2.oneshot(req).await.unwrap();
    assert!(res.status().is_success());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["matched_records"], 4);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["ids"].as_array().unwrap().len(), 2);
}
