//! TUI agent panel actor: aggregates background agent and tool feed items.
//!
//! Accepts [`crate::actors::tui_agent_panel::tui_agent_panel_ops::AgentPanelCmd::AgentFeed`] and
//! [`crate::actors::tui_agent_panel::tui_agent_panel_ops::AgentPanelCmd::ToolFeed`] commands
//! and forwards both as a unified [`AgentFeedOutput`] stream to the TUI panel.
//! Also maintains an [`AgentFeedState`] watch channel that accumulates a
//! simplified view of agent events for snapshot reads.

use super::handle::TuiAgentPanelHandle;
use super::tui_agent_panel_actor_ops as actor_ops;
use crate::domain::tui_state::AgentFeedState;
use augur_domain::domain::types::AgentFeedOutput;
use tokio::sync::{mpsc, watch};

/// Configuration for spawning the TUI agent panel actor.
///
/// `unified_tx` is the sink for all forwarded feed items. `capacity` sets the
/// command channel buffer size; use `TUI_FEED_CAPACITY.inner()` at call sites.
pub struct TuiAgentPanelConfig {
    /// Sink channel for the unified agent feed output stream.
    pub unified_tx: mpsc::Sender<AgentFeedOutput>,
    /// Command channel buffer capacity.
    pub capacity: usize,
}

/// Spawn the TUI agent panel actor and return a join handle plus a `TuiAgentPanelHandle`.
///
/// Creates a `watch::channel` seeded with a default `AgentFeedState` and an
/// `mpsc::channel` with `config.capacity` for commands. The actor task loops
/// over commands, updates accumulated state, and forwards feed items to
/// `config.unified_tx`. Returns `(JoinHandle, TuiAgentPanelHandle)`.
pub fn spawn(config: TuiAgentPanelConfig) -> (tokio::task::JoinHandle<()>, TuiAgentPanelHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(config.capacity);
    let (state_tx, state_rx) = watch::channel(AgentFeedState::default());
    let handle = TuiAgentPanelHandle::new(cmd_tx, state_rx);
    let join = tokio::spawn(actor_ops::run(cmd_rx, config.unified_tx, state_tx));
    (join, handle)
}
