//! Tests for memory store.

use pomfret::store::MemoryStore;

#[tokio::test]
async fn push_and_get() {
    let store = MemoryStore::new(10);
    let r = pomfret::store::RequestRecord::new(
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
    let id = r.id.clone();
    store.push(r).await;
    let got = store.get(&id).await.unwrap();
    assert_eq!(got.method, "POST");
    assert_eq!(got.path, "/v1/chat/completions");
    assert_eq!(got.backend_id.as_deref(), Some("ollama"));
    assert_eq!(got.model.as_deref(), Some("llama2"));
    assert!(got.response_body.is_none());
}

#[tokio::test]
async fn update_response() {
    let store = MemoryStore::new(10);
    let r = pomfret::store::RequestRecord::new(
        "POST".to_string(),
        "/v1/chat/completions".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let id = r.id.clone();
    store.push(r).await;
    store
        .update_response(&id, Some("{\"choices\":[]}".to_string()), Some(200), None)
        .await;
    let got = store.get(&id).await.unwrap();
    assert_eq!(got.response_body.as_deref(), Some("{\"choices\":[]}"));
    assert_eq!(got.status, Some(200));
}

#[tokio::test]
async fn list_recent_and_eviction() {
    let store = MemoryStore::new(3);
    for i in 0..5 {
        let r = pomfret::store::RequestRecord::new(
            "POST".to_string(),
            "/v1/chat/completions".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(i.to_string()),
        );
        store.push(r).await;
    }
    let list = store.list(10).await;
    assert_eq!(list.len(), 3, "max_len=3 so only last 3 kept");
    assert_eq!(list[0].request_body.as_deref(), Some("4"));
    assert_eq!(list[1].request_body.as_deref(), Some("3"));
    assert_eq!(list[2].request_body.as_deref(), Some("2"));
}
