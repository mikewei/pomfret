//! Lazy per-backend-id cache of [`LlmProvider`] instances.

use crate::config::BackendConfig;
use crate::providers::{create_provider, LlmProvider};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// In-memory pool of providers keyed by `BackendConfig::id`.
///
/// Cleared when backends are mutated via the console API so updated URLs/keys take effect.
#[derive(Clone)]
pub struct ProviderPool {
    inner: Arc<Mutex<HashMap<String, Arc<dyn LlmProvider>>>>,
}

impl Default for ProviderPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderPool {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Drop all cached providers (e.g. after backends config changes).
    pub fn clear(&self) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.clear();
    }

    /// Return an existing provider for `config.id` or create and insert one.
    pub fn get_or_create(
        &self,
        config: BackendConfig,
    ) -> Result<Arc<dyn LlmProvider>, Box<dyn std::error::Error + Send + Sync>> {
        let id = config.id.clone();
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = g.get(&id) {
            return Ok(Arc::clone(p));
        }
        let boxed = create_provider(config)?;
        let arc: Arc<dyn LlmProvider> = Arc::from(boxed);
        g.insert(id, Arc::clone(&arc));
        Ok(arc)
    }
}
