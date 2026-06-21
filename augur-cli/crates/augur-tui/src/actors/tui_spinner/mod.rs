//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI spinner actor module.
//!
//! Owns spinner animation state and label text for named panel targets.
//! Publishes state snapshots over a watch channel and processes commands
//! over an mpsc channel.

pub mod handle;
/// Actor task that owns spinner state and processes commands.
pub mod tui_spinner_actor;
/// Public handle for reading snapshots and sending commands.
mod tui_spinner_actor_ops;
/// Command and state types for the spinner actor.
pub mod tui_spinner_ops;

pub use handle::TuiSpinnerHandle;
