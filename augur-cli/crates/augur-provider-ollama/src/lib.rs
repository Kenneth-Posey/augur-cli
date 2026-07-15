//! Ollama provider crate.

extern crate self as augur_provider_ollama;

pub use augur_provider_shared::stream_ollama_complete as stream_complete;

// ── Test discovery stub (rust-analyzer visibility) ─────────────────────────
#[cfg(test)]
#[path = "../tests/ollama.tests.rs"]
mod ollama_tests;
