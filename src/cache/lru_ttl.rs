//! Bounded in-memory cache: LRU eviction by insertion order and per-entry TTL.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Thread-safe LRU cache with TTL per entry.
#[derive(Clone)]
pub struct LruTtlCache<K, V> {
    inner: Arc<Mutex<Inner<K, V>>>,
    max_entries: usize,
    ttl: Duration,
}

struct Inner<K, V> {
    map: HashMap<K, Entry<V>>,
    /// Oldest insertion first; used to evict when at capacity.
    lru: VecDeque<K>,
}

struct Entry<V> {
    value: V,
    expires_at: Instant,
}

impl<K: Eq + Hash + Clone, V: Clone> LruTtlCache<K, V> {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                map: HashMap::new(),
                lru: VecDeque::new(),
            })),
            max_entries,
            ttl,
        }
    }

    pub fn put(&self, key: K, value: V) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        g.remove_expired(now);

        if g.map.contains_key(&key) {
            g.lru.retain(|k| k != &key);
        } else {
            while self.max_entries > 0 && g.map.len() >= self.max_entries {
                match g.lru.pop_front() {
                    Some(old) => {
                        g.map.remove(&old);
                    }
                    None => break,
                }
            }
        }

        g.map.insert(
            key.clone(),
            Entry {
                value,
                expires_at: now + self.ttl,
            },
        );
        g.lru.push_back(key);
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        g.remove_expired(now);
        let e = g.map.get(key)?;
        if e.expires_at <= now {
            g.remove_key(key);
            return None;
        }
        Some(e.value.clone())
    }
}

impl<K: Eq + Hash + Clone, V> Inner<K, V> {
    fn remove_key(&mut self, key: &K) {
        self.map.remove(key);
        self.lru.retain(|k| k != key);
    }

    fn remove_expired(&mut self, now: Instant) {
        let expired: Vec<K> = self
            .map
            .iter()
            .filter(|(_, e)| e.expires_at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        for k in expired {
            self.remove_key(&k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip() {
        let c = LruTtlCache::<String, String>::new(256, Duration::from_secs(3600));
        c.put("call_a".to_string(), "sig1".to_string());
        assert_eq!(c.get(&"call_a".to_string()).as_deref(), Some("sig1"));
    }

    #[test]
    fn get_missing_returns_none() {
        let c = LruTtlCache::<String, String>::new(256, Duration::from_secs(3600));
        assert!(c.get(&"nope".to_string()).is_none());
    }

    #[test]
    fn evicts_oldest_when_over_capacity() {
        const N: usize = 256;
        let c = LruTtlCache::<String, String>::new(N, Duration::from_secs(3600));
        for i in 0..N {
            c.put(format!("id{i}"), format!("sig{i}"));
        }
        assert!(c.get(&"id0".to_string()).is_some());
        c.put("id_new".to_string(), "sig_new".to_string());
        assert!(c.get(&"id0".to_string()).is_none(), "oldest should be evicted");
        assert_eq!(c.get(&"id_new".to_string()).as_deref(), Some("sig_new"));
    }

    #[test]
    fn update_refreshes_entry() {
        let c = LruTtlCache::<String, String>::new(256, Duration::from_secs(3600));
        c.put("x".to_string(), "a".to_string());
        c.put("x".to_string(), "b".to_string());
        assert_eq!(c.get(&"x".to_string()).as_deref(), Some("b"));
    }
}
