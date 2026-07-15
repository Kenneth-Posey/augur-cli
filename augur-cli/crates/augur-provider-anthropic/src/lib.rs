//! Anthropic provider crate.

extern crate self as augur_provider_anthropic;

pub use augur_provider_shared::stream_anthropic_complete as stream_complete;

// ── Test discovery stub (rust-analyzer visibility) ─────────────────────────
#[cfg(test)]
#[path = "../tests/anthropic.tests.rs"]
mod anthropic_tests;
