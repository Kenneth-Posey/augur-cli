//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI chat-menu actor module.
//!
//! Owns chat-menu visibility, item contents, and the action bound to the current
//! selection. Publishes state snapshots over a watch channel and processes
//! commands over an mpsc channel.

/// Public handle for reading snapshots and sending commands.
pub mod handle;
/// Actor task that owns chat-menu state and processes commands.
pub mod tui_chat_menu_actor;
mod tui_chat_menu_actor_ops;
/// Command and state types for the chat-menu actor.
pub mod tui_chat_menu_ops;

pub use handle::TuiChatMenuHandle;
