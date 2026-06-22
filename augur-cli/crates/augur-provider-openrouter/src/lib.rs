//! OpenRouter-backed provider crate for model access and provider-owned actors.

/// OpenRouter-specific message compaction and token estimation utilities.
pub mod compaction;

/// Per-model configuration resolution from provider catalog YAML files.
pub mod model_config;

/// Provider-specific actor wiring exposed by this crate.
pub mod actors;
