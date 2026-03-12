//! In-memory store for relayed requests (for console UI).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// One recorded request: metadata + request/response bodies (or summary).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRecord {
    pub id: String,
    pub method: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_headers: Option<String>,
    pub backend_id: Option<String>,
    pub model: Option<String>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<String>,
    pub created_at: f64,
}

impl RequestRecord {
    pub fn new(
        method: String,
        path: String,
        request_query: Option<String>,
        request_headers: Option<String>,
        backend_id: Option<String>,
        model: Option<String>,
        request_body: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            method,
            path,
            request_query,
            request_headers,
            backend_id,
            model,
            request_body,
            response_body: None,
            status: None,
            response_headers: None,
            created_at: now_secs(),
        }
    }

    pub fn set_response(
        &mut self,
        body: Option<String>,
        status: Option<u16>,
        response_headers: Option<String>,
    ) {
        self.response_body = body;
        self.status = status;
        self.response_headers = response_headers;
    }
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// In-memory store with a fixed max size (FIFO eviction).
#[derive(Clone)]
pub struct MemoryStore {
    inner: Arc<RwLock<MemoryStoreInner>>,
}

struct MemoryStoreInner {
    records: VecDeque<RequestRecord>,
    max_len: usize,
}

impl MemoryStore {
    pub fn new(max_len: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(MemoryStoreInner {
                records: VecDeque::new(),
                max_len,
            })),
        }
    }

    /// Append a record; may evict oldest.
    pub async fn push(&self, record: RequestRecord) {
        let mut g = self.inner.write().await;
        if g.records.len() >= g.max_len {
            g.records.pop_front();
        }
        g.records.push_back(record);
    }

    /// Get record by id.
    pub async fn get(&self, id: &str) -> Option<RequestRecord> {
        let g = self.inner.read().await;
        g.records.iter().find(|r| r.id == id).cloned()
    }

    /// List recent records (newest first).
    pub async fn list(&self, limit: usize) -> Vec<RequestRecord> {
        let g = self.inner.read().await;
        let n = g.records.len();
        let start = n.saturating_sub(limit);
        g.records
            .range(start..)
            .rev()
            .cloned()
            .take(limit)
            .collect()
    }

    /// Update existing record (e.g. set response).
    pub async fn update_response(
        &self,
        id: &str,
        response_body: Option<String>,
        status: Option<u16>,
        response_headers: Option<String>,
    ) {
        let mut g = self.inner.write().await;
        if let Some(r) = g.records.iter_mut().find(|r| r.id == id) {
            r.set_response(response_body, status, response_headers);
        }
    }

    /// Stats: total count and per-backend count + last request time.
    pub async fn get_stats(&self) -> StoreStats {
        let g = self.inner.read().await;
        let total = g.records.len();
        let mut by_backend: std::collections::HashMap<String, BackendStats> =
            std::collections::HashMap::new();
        for r in g.records.iter() {
            let bid = r.backend_id.as_deref().unwrap_or("_none");
            let entry = by_backend.entry(bid.to_string()).or_insert(BackendStats {
                count: 0,
                last_at: 0.0,
            });
            entry.count += 1;
            if r.created_at > entry.last_at {
                entry.last_at = r.created_at;
            }
        }
        StoreStats {
            total,
            by_backend,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StoreStats {
    pub total: usize,
    pub by_backend: std::collections::HashMap<String, BackendStats>,
}

#[derive(Debug, Clone, Default)]
pub struct BackendStats {
    pub count: usize,
    pub last_at: f64,
}
