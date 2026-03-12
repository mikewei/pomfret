//! Pomfret library: Proxy Of Models For Routing, Evaluation & Telemetry — OpenAI-compatible proxy, backend routing, request store.
//!
//! All business logic lives here so it can be tested from `tests/` and used from `main.rs`.

pub mod config;
pub mod embed;
pub mod providers;
pub mod proxy;
pub mod routing;
pub mod store;
pub mod web;
