//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Token-tracker actor: sole owner of in-memory LLM token accumulation.
//!
//! Receives `LlmUsage` events from all sources (main conversation and background
//! pipeline agents), and accumulates running totals for the current process.
//!
//! Use [`crate::actors::token_tracker::spawn`] to create the actor and obtain a [`TokenTrackerHandle`].
//! All callers send commands through the handle; the actor serializes all
//! mutations so no shared-mutex concurrency is required.

pub mod handle;
pub mod token_tracker_actor;
mod token_tracker_actor_ops;
pub mod token_tracker_ops;

pub use handle::TokenTrackerHandle;
pub use token_tracker_actor::spawn;

/// Spawn the token-tracker actor with explicit initial settings and optional
/// persistence path.
pub fn spawn_with_settings(
    initial_settings: crate::token_history::ProjectSettings,
    settings_path: Option<std::path::PathBuf>,
) -> (tokio::task::JoinHandle<()>, TokenTrackerHandle) {
    token_tracker_actor::spawn_with_settings(initial_settings, settings_path)
}
