//! Provider tests using wiremock to mock upstream.

use bytes::Bytes;
use pomfret::config::{BackendConfig, BackendType};
use pomfret::providers::{create_provider, ProviderError, ProviderResponse};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn chat_completions_non_stream_returns_body() {
    let server = MockServer::start().await;
    let config = BackendConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: format!("{}/v1", server.uri()),
        api_key: None,
        backend_type: BackendType::Ollama,
        model: None,
    };
    let provider = create_provider(config).unwrap();

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"choices":[{"message":{"content":"Hi"}}]}"#.as_bytes(), "application/json"))
        .mount(&server)
        .await;

    let body = Bytes::from(r#"{"model":"llama2","messages":[{"role":"user","content":"Hi"}]}"#);
    let res = provider.chat_completions(body, false).await.unwrap();
    match &res {
        ProviderResponse::Body(b) => {
            let s = std::str::from_utf8(b).unwrap();
            assert!(s.contains("\"choices\""));
            assert!(s.contains("Hi"));
        }
        _ => panic!("expected Body"),
    }
}

#[tokio::test]
async fn chat_completions_sends_auth_header_when_api_key_set() {
    let server = MockServer::start().await;
    let config = BackendConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: format!("{}/v1", server.uri()),
        api_key: Some("sk-secret".to_string()),
        backend_type: BackendType::OpenAiCompat,
        model: None,
    };
    let provider = create_provider(config).unwrap();

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(wiremock::matchers::header("Authorization", "Bearer sk-secret"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(b"{}", "application/json"))
        .mount(&server)
        .await;

    let body = Bytes::from(r#"{"model":"gpt-4","messages":[]}"#);
    let _ = provider.chat_completions(body, false).await.unwrap();
}

#[tokio::test]
async fn chat_completions_returns_error_on_4xx() {
    let server = MockServer::start().await;
    let config = BackendConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        base_url: format!("{}/v1", server.uri()),
        api_key: None,
        backend_type: BackendType::Ollama,
        model: None,
    };
    let provider = create_provider(config).unwrap();

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(422).set_body_raw(
                r#"{"error":"invalid model"}"#.as_bytes(),
                "application/json",
            ),
        )
        .mount(&server)
        .await;

    let body = Bytes::from(r#"{"model":"nonexistent","messages":[]}"#);
    let res = provider.chat_completions(body, false).await;
    match res {
        Err(ProviderError::Status(code, body)) => {
            assert_eq!(code.as_u16(), 422);
            assert!(body.contains("invalid model"));
        }
        _ => panic!("expected Status(422) error"),
    }
}
