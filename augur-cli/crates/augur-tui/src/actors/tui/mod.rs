//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI actor shell modules: actor runtime, assistant helpers, and public handle.
//!
//! Manages the terminal UI presentation layer, displaying agent state, handling
//! keyboard input, and rendering messages and tool results. Coordinates with the
//! agent actor through message channels and maintains the visual state of the
//! application.

/// Focused helper modules used by the TUI actor runtime.
pub mod assistant;
/// Public handle for interacting with the running TUI actor.
pub mod handle;
/// TUI actor runtime and event loop orchestration.
pub mod tui_actor;
mod tui_actor_ops;
