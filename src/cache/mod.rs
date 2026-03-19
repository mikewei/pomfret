//! Shared cache primitives (generic LRU + TTL).

pub(crate) mod lru_ttl;

pub(crate) use lru_ttl::LruTtlCache;
