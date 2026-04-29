//! End-to-end tests for Gemini `thought_signature` via server-side cache (tool_call id or `SIGID:` in `<think>`).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pomfret::config::{AppState, BackendConfig, BackendType, Config};
use pomfret::store::MemoryStore;
use pomfret::web::{router, NotifyEvent, ProviderPool, WebState};
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
        provider_pool: ProviderPool::new(),
        backend_timeout_secs: 300,
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

#[tokio::test]
async fn gemini_non_stream_caches_sig_and_strips_from_client_body() {
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

    assert!(
        resp["choices"][0]["message"]["tool_calls"][0]
            .get("extra_content")
            .is_none(),
        "thought_signature should be stripped from client response"
    );
    let content = &resp["choices"][0]["message"]["content"];
    assert!(
        content.is_null() || content.as_str() == Some(""),
        "content should not get TSIG markers, got {:?}",
        content
    );

    let records = store.list(10).await;
    assert_eq!(records.len(), 1);
    let stored = records[0].response_body.as_ref().unwrap();
    assert!(
        !stored.contains("thought_signature"),
        "stored reconstruction should not contain raw signature"
    );
}

#[tokio::test]
async fn gemini_message_level_sigid_roundtrip() {
    let mock = MockServer::start().await;
    let config = gemini_config(&mock.uri());
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store);
    let app = router(state);

    let first_upstream = serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "Plain reply",
                "extra_content": { "google": { "thought_signature": "SIG_PLAIN" } }
            },
            "finish_reason": "stop"
        }]
    });

    Mock::given(method("POST"))
        .and(path("/v1beta/openai/chat/completions"))
        .and(body_string_contains("thought_signature"))
        .and(body_string_contains("SIG_PLAIN"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"choices":[{"message":{"role":"assistant","content":"Second."}}]}"#.as_bytes(),
            "application/json",
        ))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1beta/openai/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            serde_json::to_vec(&first_upstream).unwrap(),
            "application/json",
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let req1 = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"model":"gemini-3","messages":[{"role":"user","content":"Hi"}]}"#,
        ))
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let bytes1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let resp1: serde_json::Value = serde_json::from_slice(&bytes1).unwrap();
    let content1 = resp1["choices"][0]["message"]["content"]
        .as_str()
        .expect("string content with SIGID");
    assert!(
        content1.contains("Plain reply") && content1.contains("SIGID:tsig_"),
        "expected SIGID in content: {:?}",
        content1
    );
    assert!(
        content1.contains("<think>") && content1.contains("</think>"),
        "expected think-wrapped SIGID: {:?}",
        content1
    );
    assert!(
        resp1["choices"][0]["message"]
            .get("extra_content")
            .is_none(),
        "extra_content should be stripped"
    );

    let req_body2 = serde_json::json!({
        "model": "gemini-3",
        "messages": [
            { "role": "user", "content": "Again" },
            { "role": "assistant", "content": content1 }
        ]
    });

    let req2 = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body2).unwrap()))
        .unwrap();
    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let bytes2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(std::str::from_utf8(&bytes2).unwrap().contains("Second."));
}

#[tokio::test]
async fn gemini_second_request_injects_cached_signature_by_tool_call_id() {
    let mock = MockServer::start().await;
    let config = gemini_config(&mock.uri());
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store);
    let app = router(state);

    let first_upstream = serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "extra_content": { "google": { "thought_signature": "SIG_ALPHA" } },
                    "function": { "name": "check_flight", "arguments": "{}" },
                    "id": "fc-1",
                    "type": "function"
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });

    // Second round: upstream must receive thought_signature from cache.
    Mock::given(method("POST"))
        .and(path("/v1beta/openai/chat/completions"))
        .and(body_string_contains("thought_signature"))
        .and(body_string_contains("SIG_ALPHA"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"choices":[{"message":{"role":"assistant","content":"Done."}}]}"#.as_bytes(),
            "application/json",
        ))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1beta/openai/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            serde_json::to_vec(&first_upstream).unwrap(),
            "application/json",
        ))
        .expect(1)
        .mount(&mock)
        .await;

    let req1 = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"model":"gemini-3","messages":[{"role":"user","content":"Hi"}]}"#,
        ))
        .unwrap();
    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);

    let req_body2 = serde_json::json!({
        "model": "gemini-3",
        "messages": [
            { "role": "user", "content": "Check flight AA100" },
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "function": { "name": "check_flight", "arguments": "{}" },
                    "id": "fc-1",
                    "type": "function"
                }]
            },
            {
                "role": "tool",
                "content": "{\"status\":\"delayed\"}",
                "tool_call_id": "fc-1"
            }
        ]
    });

    let req2 = Request::builder()
        .method("POST")
        .uri("/v1/chat/completions")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body2).unwrap()))
        .unwrap();
    let res2 = app.oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res2.into_body(), usize::MAX).await.unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("Done."));
}

#[tokio::test]
async fn non_gemini_backend_does_not_touch_signatures() {
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
    assert!(s.contains("Hello"));
}

#[tokio::test]
async fn gemini_stream_caches_sig_and_strips_from_sse() {
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
            ResponseTemplate::new(200).set_body_raw(sse_body.into_bytes(), "text/event-stream"),
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
        !output.contains("thought_signature"),
        "SSE should strip thought_signature:\n{}",
        output
    );
    assert!(
        !output.contains("STREAM_SIG"),
        "raw signature should not appear in client SSE:\n{}",
        output
    );
    assert!(output.contains("[DONE]"), "stream should end with [DONE]");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let records = store.list(10).await;
    assert_eq!(records.len(), 1);
    let stored = records[0].response_body.as_ref().unwrap();
    assert!(
        !stored.contains("thought_signature"),
        "reconstructed store body should not contain signature: {}",
        stored
    );
}
