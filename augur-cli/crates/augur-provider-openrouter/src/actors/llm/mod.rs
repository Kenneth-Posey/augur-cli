//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! LLM actor and provider implementations.
//!
//! Manages interaction with language model providers (Claude, GPT, local models).
//! Handles streaming responses, token counting, and model selection. Provides
//! the ChatProvider trait implementation used by the agent actor.

/// Cloneable LLM handle and re-exported client trait.
pub mod handle;
/// LLM actor task lifecycle and dispatch loop.
pub mod llm_actor;
/// Private helper operations delegated from `actor`.
mod llm_actor_ops;
/// Provider-specific streaming backends.
pub mod providers;
