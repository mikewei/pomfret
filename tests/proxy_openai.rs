//! Proxy layer tests: POST /v1/chat/completions forwards to backend and records to store.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pomfret::config::{AppState, BackendConfig, BackendType, Config};
use pomfret::store::MemoryStore;
use pomfret::web::{router, NotifyEvent, ProviderPool, WebState};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tower::ServiceExt;

fn test_web_state(app_state: AppState, store: MemoryStore, backend_timeout_secs: u64) -> WebState {
    let (notify_tx, _) = broadcast::channel::<NotifyEvent>(32);
    WebState {
        app_state,
        store,
        backends_path: PathBuf::from("/tmp/pomfret-test-backends.conf"),
        routing_path: PathBuf::from("/tmp/pomfret-test-routing.conf"),
        notify_tx,
        provider_pool: ProviderPool::new(),
        backend_timeout_secs,
    }
}
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn chat_completions_503_when_no_backend() {
    let config = Config::default_empty();
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store, 300);
    let app = router(state);

    let body = r#"{"model":"llama2","messages":[{"role":"user","content":"Hi"}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("No backend available"));
}

#[tokio::test]
async fn chat_completions_forwards_to_backend_and_records() {
    let mock = MockServer::start().await;
    let backend = BackendConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: format!("{}/v1", mock.uri()),
        api_key: None,
        backend_type: BackendType::Ollama,
        model: None,
    };
    let config = Config {
        backends: vec![backend],
    };
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store.clone(), 300);
    let app = router(state);

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"choices":[{"message":{"content":"Hello"}}]}"#.as_bytes(),
                "application/json",
            ),
        )
        .mount(&mock)
        .await;

    let body = r#"{"model":"llama2","messages":[{"role":"user","content":"Hi"}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("Hello"));

    let list = store.list(10).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].method, "POST");
    assert_eq!(list[0].path, "/v1/chat/completions");
    assert_eq!(list[0].backend_id.as_deref(), Some("test"));
    assert_eq!(list[0].status, Some(200));
    assert!(list[0].response_body.as_ref().unwrap().contains("Hello"));
}

#[tokio::test]
async fn get_models_forwards_to_backend() {
    let mock = MockServer::start().await;
    let backend = BackendConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: format!("{}/v1", mock.uri()),
        api_key: None,
        backend_type: BackendType::Ollama,
        model: None,
    };
    let config = Config {
        backends: vec![backend],
    };
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store, 300);
    let app = router(state);

    wiremock::Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_raw(
                r#"{"data":[{"id":"llama2"}]}"#.as_bytes(),
                "application/json",
            ),
        )
        .mount(&mock)
        .await;

    let req = Request::builder()
        .method("GET")
        .uri("/v1/models")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("llama2"));
}

#[tokio::test]
async fn get_models_503_when_no_backend() {
    let config = Config::default_empty();
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let app = router(test_web_state(app_state, store, 300));

    let req = Request::builder()
        .method("GET")
        .uri("/v1/models")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn chat_completions_pomfret_timeout_maps_504_with_marker() {
    let mock = MockServer::start().await;
    let backend = BackendConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: format!("{}/v1", mock.uri()),
        api_key: None,
        backend_type: BackendType::Ollama,
        model: None,
    };
    let config = Config {
        backends: vec![backend],
    };
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store.clone(), 1);
    let app = router(state);

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(3))
                .set_body_raw(
                    r#"{"choices":[{"message":{"content":"late"}}]}"#.as_bytes(),
                    "application/json",
                ),
        )
        .mount(&mock)
        .await;

    let body = r#"{"model":"llama2","messages":[{"role":"user","content":"Hi"}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::GATEWAY_TIMEOUT);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("(pomfret)"),
        "expected Pomfret marker in message: {msg}"
    );

    let list = store.list(10).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, Some(504));
    let stored = list[0].response_body.as_ref().unwrap();
    assert!(stored.contains("(pomfret)"));
}

#[tokio::test]
async fn chat_completions_upstream_504_passthrough_no_pomfret_marker() {
    let mock = MockServer::start().await;
    let backend = BackendConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: format!("{}/v1", mock.uri()),
        api_key: None,
        backend_type: BackendType::Ollama,
        model: None,
    };
    let config = Config {
        backends: vec![backend],
    };
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store.clone(), 300);
    let app = router(state);

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(504).set_body_string("upstream rate limited"))
        .mount(&mock)
        .await;

    let body = r#"{"model":"llama2","messages":[{"role":"user","content":"Hi"}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::from_u16(504).unwrap());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(
        !s.contains("(pomfret)"),
        "upstream passthrough must not add Pomfret marker: {s}"
    );
    assert!(s.contains("upstream rate limited"));

    let list = store.list(10).await;
    assert_eq!(list[0].status, Some(504));
    assert!(!list[0].response_body.as_ref().unwrap().contains("(pomfret)"));
}
