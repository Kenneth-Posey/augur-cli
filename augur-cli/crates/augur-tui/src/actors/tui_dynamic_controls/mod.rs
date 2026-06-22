//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI dynamic controls actor module.
//!
//! Owns the runtime key-hint panel state, which changes based on the active UI
//! mode. Publishes state snapshots over a watch channel and processes commands
//! over an mpsc channel.

pub mod handle;
/// Actor task that owns dynamic controls state and processes commands.
pub mod tui_dynamic_controls_actor;
/// Public handle for reading snapshots and sending commands.
mod tui_dynamic_controls_actor_ops;
/// Command and state types for the dynamic controls actor.
pub mod tui_dynamic_controls_ops;

pub use handle::TuiDynamicControlsHandle;
