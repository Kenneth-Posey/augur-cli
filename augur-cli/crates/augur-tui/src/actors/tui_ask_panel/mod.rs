//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI ask panel actor: side-channel ask panel state management.
//!
//! Tracks whether the ask panel is open and accumulates its output lines.
//! The watch channel holds `None` (closed) or `Some(AskPanelState)` (open).
//! Callers control the panel via [`crate::actors::tui_ask_panel::TuiAskPanelHandle`].

pub mod handle;
pub mod tui_ask_panel_actor;
mod tui_ask_panel_actor_ops;
pub mod tui_ask_panel_ops;

pub use handle::TuiAskPanelHandle;
