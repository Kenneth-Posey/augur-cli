//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI main feed panel actor: feed-aggregation for main agent, ask-panel, and orchestrator events.
//!
//! Accepts main agent output, ask-panel output, and deterministic orchestrator events
//! and forwards them as a unified [`crate::actors::tui_main_feed_panel::tui_main_feed_panel_ops::MainFeedItem`] stream for the TUI main
//! conversation panel.

pub mod handle;
pub mod tui_main_feed_panel_actor;
mod tui_main_feed_panel_actor_ops;
pub mod tui_main_feed_panel_ops;

pub use handle::TuiMainFeedPanelHandle;
