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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_name: Option<String>,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_model: Option<String>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<String>,
    pub created_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u64>,
}

impl RequestRecord {
    pub fn new(
        method: String,
        path: String,
        request_query: Option<String>,
        request_headers: Option<String>,
        backend_id: Option<String>,
        backend_name: Option<String>,
        model: Option<String>,
        backend_model: Option<String>,
        request_body: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            method,
            path,
            request_query,
            request_headers,
            backend_id,
            backend_name,
            model,
            backend_model,
            request_body,
            response_body: None,
            status: None,
            response_headers: None,
            created_at: now_secs(),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
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

    /// Update token usage for a request record.
    pub async fn update_tokens(
        &self,
        id: &str,
        prompt_tokens: Option<u64>,
        completion_tokens: Option<u64>,
        total_tokens: Option<u64>,
    ) {
        let mut g = self.inner.write().await;
        if let Some(r) = g.records.iter_mut().find(|r| r.id == id) {
            r.prompt_tokens = prompt_tokens;
            r.completion_tokens = completion_tokens;
            r.total_tokens = total_tokens;
        }
    }

    /// Stats with optional time filter. `since` is epoch seconds; only records >= since are counted.
    pub async fn get_stats(&self, since: Option<f64>) -> StoreStats {
        let g = self.inner.read().await;
        let mut total: usize = 0;
        let mut total_prompt_tokens: u64 = 0;
        let mut total_completion_tokens: u64 = 0;
        let mut total_tokens: u64 = 0;
        let mut by_backend: std::collections::HashMap<String, BackendStats> =
            std::collections::HashMap::new();
        for r in g.records.iter() {
            if let Some(s) = since {
                if r.created_at < s {
                    continue;
                }
            }
            total += 1;
            let bid = r.backend_id.as_deref().unwrap_or("_none");
            let entry = by_backend
                .entry(bid.to_string())
                .or_insert_with(BackendStats::default);
            entry.count += 1;
            if r.created_at > entry.last_at {
                entry.last_at = r.created_at;
            }
            let pt = r.prompt_tokens.unwrap_or(0);
            let ct = r.completion_tokens.unwrap_or(0);
            let tt = r.total_tokens.unwrap_or(0);
            entry.prompt_tokens += pt;
            entry.completion_tokens += ct;
            entry.total_tokens += tt;
            total_prompt_tokens += pt;
            total_completion_tokens += ct;
            total_tokens += tt;
        }
        StoreStats {
            total,
            total_prompt_tokens,
            total_completion_tokens,
            total_tokens,
            by_backend,
        }
    }

    /// Time-series data: buckets of `bucket_secs` width over the last `hours` hours.
    pub async fn get_timeseries(&self, hours: u64, bucket_secs: u64) -> Vec<TimeseriesBucket> {
        let now = now_secs();
        let now_bucket = (now as u64 / bucket_secs) * bucket_secs;
        let num_buckets = (hours * 3600 / bucket_secs) as usize;
        let first_bucket = now_bucket - (num_buckets as u64 - 1) * bucket_secs;

        let mut buckets: Vec<TimeseriesBucket> = (0..num_buckets)
            .map(|i| TimeseriesBucket {
                ts: (first_bucket + i as u64 * bucket_secs) as f64,
                requests: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            })
            .collect();

        let g = self.inner.read().await;
        let since = first_bucket as f64;
        for r in g.records.iter() {
            if r.created_at < since {
                continue;
            }
            let r_bucket = (r.created_at as u64 / bucket_secs) * bucket_secs;
            if r_bucket < first_bucket {
                continue;
            }
            let idx = ((r_bucket - first_bucket) / bucket_secs) as usize;
            if idx < num_buckets {
                buckets[idx].requests += 1;
                buckets[idx].prompt_tokens += r.prompt_tokens.unwrap_or(0);
                buckets[idx].completion_tokens += r.completion_tokens.unwrap_or(0);
                buckets[idx].total_tokens += r.total_tokens.unwrap_or(0);
            }
        }
        buckets
    }
}

#[derive(Debug, Clone, Default)]
pub struct StoreStats {
    pub total: usize,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
    pub by_backend: std::collections::HashMap<String, BackendStats>,
}

#[derive(Debug, Clone, Default)]
pub struct BackendStats {
    pub count: usize,
    pub last_at: f64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeseriesBucket {
    pub ts: f64,
    pub requests: usize,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}
