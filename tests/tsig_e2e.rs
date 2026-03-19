//! End-to-end tests for TSIG (thought-signature) encoding/decoding through
//! the full gateway stack (POST /v1/chat/completions with a Gemini backend).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pomfret::config::{AppState, BackendConfig, BackendType, Config};
use pomfret::store::MemoryStore;
use pomfret::web::{router, NotifyEvent, WebState};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tower::ServiceExt;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_web_state(app_state: AppState, store: MemoryStore) -> WebState {
    let (notify_tx, _) = broadcast::channel::<NotifyEvent>(32);
    WebState {
        app_state,
        store,
        backends_path: PathBuf::from("/tmp/pomfret-tsig-test-backends.conf"),
        routing_path: PathBuf::from("/tmp/pomfret-tsig-test-routing.conf"),
        notify_tx,
    }
}

fn gemini_config(mock_uri: &str) -> Config {
    Config {
        backends: vec![BackendConfig {
            id: "gemini-test".to_string(),
            name: "Gemini".to_string(),
            base_url: mock_uri.to_string(),
            api_key: Some("AIzaFAKE".to_string()),
            backend_type: BackendType::Gemini,
            model: None,
        }],
    }
}

// ---- response path: non-streaming ----

#[tokio::test]
async fn gemini_non_stream_injects_tsig_into_content() {
    let mock = MockServer::start().await;
    let config = gemini_config(&mock.uri());
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store.clone());
    let app = router(state);

    let upstream_body = serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "extra_content": {
                        "google": { "thought_signature": "SIG_ALPHA" }
                    },
                    "function": { "name": "check_flight", "arguments": "{\"flight\":\"AA100\"}" },
                    "id": "fc-1",
                    "type": "function"
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
    });

    Mock::given(method("POST"))
        .and(path("/v1beta/openai/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(serde_json::to_vec(&upstream_body).unwrap(), "application/json"),
        )
        .mount(&mock)
        .await;

    let req_body = r#"{"model":"gemini-3","messages":[{"role":"user","content":"Check AA100"}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(req_body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let resp: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let content = resp["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(
        content.starts_with("<think>TSIG:SIG_ALPHA</think>"),
        "content should contain TSIG marker, got: {}",
        content
    );

    assert!(
        resp["choices"][0]["message"]["tool_calls"][0]
            .get("extra_content")
            .is_none(),
        "extra_content should be removed from tool_calls"
    );

    let records = store.list(10).await;
    assert_eq!(records.len(), 1);
    let stored = records[0].response_body.as_ref().unwrap();
    assert!(stored.contains("<think>TSIG:SIG_ALPHA</think>"));
}

#[tokio::test]
async fn non_gemini_backend_does_not_inject_tsig() {
    let mock = MockServer::start().await;
    let config = Config {
        backends: vec![BackendConfig {
            id: "openai-test".to_string(),
            name: "OpenAI".to_string(),
            base_url: format!("{}/v1", mock.uri()),
            api_key: None,
            backend_type: BackendType::OpenAiCompat,
            model: None,
        }],
    };
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store);
    let app = router(state);

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"choices":[{"message":{"role":"assistant","content":"Hello"}}]}"#.as_bytes(),
                "application/json",
            ),
        )
        .mount(&mock)
        .await;

    let req_body = r#"{"model":"gpt-4","messages":[{"role":"user","content":"Hi"}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(req_body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(!s.contains("TSIG"));
    assert!(s.contains("Hello"));
}

// ---- request path: decode TSIG markers ----

#[tokio::test]
async fn gemini_request_decode_writes_back_tsig() {
    let mock = MockServer::start().await;
    let config = gemini_config(&mock.uri());
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store);
    let app = router(state);

    Mock::given(method("POST"))
        .and(path("/v1beta/openai/chat/completions"))
        .and(body_string_contains("thought_signature"))
        .and(body_string_contains("SIG_ALPHA"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"choices":[{"message":{"role":"assistant","content":"Done."}}]}"#.as_bytes(),
                "application/json",
            ),
        )
        .mount(&mock)
        .await;

    let req_body = serde_json::json!({
        "model": "gemini-3",
        "messages": [
            { "role": "user", "content": "Check flight AA100" },
            {
                "role": "assistant",
                "content": "<think>TSIG:SIG_ALPHA</think>",
                "tool_calls": [{
                    "function": { "name": "check_flight", "arguments": "{\"flight\":\"AA100\"}" },
                    "id": "fc-1", "type": "function"
                }]
            },
            {
                "role": "tool",
                "content": "{\"status\":\"delayed\"}",
                "tool_call_id": "fc-1"
            }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "upstream should have received thought_signature and matched"
    );

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("Done."));
}

#[tokio::test]
async fn non_gemini_request_strips_tsig_but_does_not_write_back() {
    let mock = MockServer::start().await;
    let config = Config {
        backends: vec![BackendConfig {
            id: "openai".to_string(),
            name: "OpenAI".to_string(),
            base_url: format!("{}/v1", mock.uri()),
            api_key: None,
            backend_type: BackendType::OpenAiCompat,
            model: None,
        }],
    };
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store);
    let app = router(state);

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                r#"{"choices":[{"message":{"role":"assistant","content":"ok"}}]}"#.as_bytes(),
                "application/json",
            ),
        )
        .mount(&mock)
        .await;

    let req_body = serde_json::json!({
        "model": "gpt-4",
        "messages": [
            { "role": "user", "content": "Hi" },
            {
                "role": "assistant",
                "content": "text<<TSIG:SIG_X>>",
                "tool_calls": [{
                    "function": { "name": "f", "arguments": "{}" },
                    "id": "fc-1", "type": "function"
                }]
            },
            { "role": "tool", "content": "{}", "tool_call_id": "fc-1" }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ---- response path: streaming ----

#[tokio::test]
async fn gemini_stream_injects_tsig_into_delta_content() {
    let mock = MockServer::start().await;
    let config = gemini_config(&mock.uri());
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store.clone());
    let app = router(state);

    let sse_body = [
        r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#,
        "",
        r#"data: {"choices":[{"delta":{"tool_calls":[{"extra_content":{"google":{"thought_signature":"STREAM_SIG"}},"function":{"name":"f","arguments":"{}"},"id":"fc-1","type":"function"}]}}]}"#,
        "",
        r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}"#,
        "",
        "data: [DONE]",
        "",
    ]
    .join("\n");

    Mock::given(method("POST"))
        .and(path("/v1beta/openai/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(sse_body.into_bytes(), "text/event-stream"),
        )
        .mount(&mock)
        .await;

    let req_body = r#"{"model":"gemini-3","stream":true,"messages":[{"role":"user","content":"Hi"}]}"#;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(req_body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let output = String::from_utf8_lossy(&bytes).to_string();

    assert!(
        output.contains("<think>TSIG:STREAM_SIG</think>"),
        "SSE output should contain TSIG marker, got:\n{}",
        output
    );
    assert!(
        !output.contains("thought_signature"),
        "raw thought_signature should be removed from SSE output:\n{}",
        output
    );
    assert!(output.contains("[DONE]"), "stream should end with [DONE]");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let records = store.list(10).await;
    assert_eq!(records.len(), 1);
    let stored = records[0].response_body.as_ref().unwrap();
    assert!(
        stored.contains("<think>TSIG:STREAM_SIG</think>"),
        "store should contain TSIG marker in reconstructed body"
    );
}
