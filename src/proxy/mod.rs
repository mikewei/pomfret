//! Proxy layer: receive OpenAI-compatible requests and forward to providers.

mod openai;

pub use openai::{handle_chat_completions, handle_models};
