//! Integration test: health check and app mounts.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pomfret::config::{AppState, Config};
use pomfret::store::MemoryStore;
use pomfret::web::{router, NotifyEvent, WebState};
use std::path::PathBuf;
use tokio::sync::broadcast;
use tower::ServiceExt;

fn test_web_state(app_state: AppState, store: MemoryStore) -> WebState {
    let (notify_tx, _) = broadcast::channel::<NotifyEvent>(32);
    WebState {
        app_state,
        store,
        backends_path: PathBuf::from("/tmp/pomfret-test-backends.conf"),
        routing_path: PathBuf::from("/tmp/pomfret-test-routing.conf"),
        notify_tx,
    }
}

#[tokio::test]
async fn health_returns_ok() {
    let config = Config::default_empty();
    let app_state = AppState::new(config);
    let store = MemoryStore::new(100);
    let state = test_web_state(app_state, store);
    let app = router(state);

    let req = Request::builder().uri("/health").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), b"ok");
}
