//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Ask-panel actor module.
//!
//! Provides a limited-capability agent actor for side-channel LLM queries
//! that do not affect main conversation history. The ask actor uses a
//! read-only tool registry (no shell_exec, no file_create) and maintains
//! isolated conversation context separate from the main agent.

pub mod ask_actor;
mod ask_actor_ops;
pub mod handle;

pub use handle::AskHandle;
