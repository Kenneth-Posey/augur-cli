//! OpenAI-compatible provider crate.

extern crate self as augur_provider_openai;

pub use augur_provider_shared::{stream_openai_compat, stream_openai_complete as stream_complete};

// ── Test discovery stub (rust-analyzer visibility) ─────────────────────────
#[cfg(test)]
#[path = "../tests/openai.tests.rs"]
mod openai_tests;
