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

// ── Test discovery stubs (rust-analyzer visibility) ────────────────────────
// Top-level #[path] stubs let VS Code / rust-analyzer discover tests in the
// external tests/ directory.  Without them, tests declared only through
// [[test]] in Cargo.toml are invisible to the IDE test explorer.
#[cfg(test)]
#[path = "../tests/lib.tests.rs"]
mod lib_tests;
#[cfg(test)]
#[path = "../tests/request_context.tests.rs"]
mod request_context_tests;
#[cfg(test)]
#[path = "../tests/retry.tests.rs"]
mod retry_tests;
#[cfg(test)]
#[path = "../tests/streaming.tests.rs"]
mod streaming_tests;
