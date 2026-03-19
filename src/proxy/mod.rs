//! Proxy layer: receive OpenAI-compatible requests and forward to providers.
//!
//! For Gemini backends, the [`tsig`] module transparently encodes/decodes
//! `thought_signature` values as `<<TSIG:…>>` markers in assistant `content`,
//! enabling multi-step function calling through standard OpenAI clients.

mod openai;
pub(crate) mod tsig;

pub use openai::{handle_chat_completions, handle_models};
