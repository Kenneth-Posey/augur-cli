//! OpenRouter-backed provider crate for model access and provider-owned actors.

extern crate self as augur_provider_openrouter;

// ── Test discovery stubs (rust-analyzer visibility) ────────────────────────
// Makes VS Code / rust-analyzer discover tests in the external tests/
// directory.  Without these, tests declared only through [[test]] in
// Cargo.toml are invisible to the IDE test explorer.
#[cfg(test)]
#[path = "../tests/actors/mod.tests.rs"]
mod actors_tests;
#[cfg(test)]
#[path = "../tests/compaction.tests.rs"]
mod compaction_tests;

/// OpenRouter-specific message compaction and token estimation utilities.
pub mod compaction;

/// Per-model configuration resolution from provider catalog YAML files.
pub mod model_config;

/// Provider-specific actor wiring exposed by this crate.
pub mod actors;
