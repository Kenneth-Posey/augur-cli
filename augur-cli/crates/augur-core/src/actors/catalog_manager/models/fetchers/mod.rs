//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Provider-specific model list fetchers.
//!
//! Each submodule exposes an async `fetch_models` function that queries its
//! respective provider API and returns a [`Vec<ModelInfo>`].

pub mod anthropic;
pub mod ollama;
pub mod openai;
