//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! TUI agent panel actor: feed-aggregation for background agent and tool message feeds.
//!
//! Accepts background agent message feeds and background tool message feeds and
//! forwards them as a unified [`augur_domain::domain::types::AgentFeedOutput`] stream for the TUI panel.

pub mod handle;
pub mod tui_agent_panel_actor;
mod tui_agent_panel_actor_ops;
pub mod tui_agent_panel_ops;

pub use handle::TuiAgentPanelHandle;
