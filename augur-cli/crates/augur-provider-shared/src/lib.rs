//! Shared LLM request, streaming, and retry helpers for provider crates.

extern crate self as augur_provider_shared;

pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod request_context;
pub mod retry;
pub mod streaming;

pub use anthropic::stream_anthropic_complete;
pub use ollama::stream_ollama_complete;
pub use openai::{stream_openai_compat, stream_openai_complete};

pub use request_context::*;
pub use retry::*;
pub use streaming::*;
