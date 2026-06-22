//! TuiAgentPanelHandle: fire-and-forget client for the TUI agent panel actor.

use super::tui_agent_panel_ops::AgentPanelCmd;
use crate::domain::tui_state::AgentFeedState;
use augur_domain::domain::types::AgentFeedOutput;
use tokio::sync::{mpsc, watch};

/// Fire-and-forget handle to the running TUI agent panel actor.
///
/// Cloning shares the same underlying actor task. Callers push feed items
/// without blocking - the actor forwards them to the unified output channel.
/// Dropping all clones closes the actor's command channel.
#[derive(Clone)]
pub struct TuiAgentPanelHandle {
    tx: mpsc::Sender<AgentPanelCmd>,
    state_rx: watch::Receiver<AgentFeedState>,
}

impl TuiAgentPanelHandle {
    /// Create a handle from a command sender and state watch receiver. Called only by `spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<AgentPanelCmd>,
        state_rx: watch::Receiver<AgentFeedState>,
    ) -> Self {
        TuiAgentPanelHandle { tx, state_rx }
    }

    /// Return the current accumulated agent feed state by reading the watch-channel snapshot.
    ///
    /// This is a momentary borrow of the watch channel's internal cell - not
    /// shared mutable state. The value reflects whatever the actor last published.
    pub fn current_state(&self) -> AgentFeedState {
        self.state_rx.borrow().clone()
    }

    /// Clone the watch receiver so the TUI runtime can subscribe to state updates.
    ///
    /// Returns a new `watch::Receiver<AgentFeedState>` that tracks the same actor.
    pub fn state_rx(&self) -> watch::Receiver<AgentFeedState> {
        self.state_rx.clone()
    }

    /// Forward a background agent feed item to the unified output channel.
    ///
    /// Inputs: `item` - the `AgentFeedOutput` event from a background agent.
    /// Side effect: silently drops the item if the actor channel is full or stopped.
    pub fn send_agent_feed(&self, item: AgentFeedOutput) {
        let _ = self.tx.try_send(AgentPanelCmd::AgentFeed(item));
    }

    /// Forward a background tool feed item to the unified output channel.
    ///
    /// Inputs: `item` - the `AgentFeedOutput` event from a background tool.
    /// Side effect: silently drops the item if the actor channel is full or stopped.
    pub fn send_tool_feed(&self, item: AgentFeedOutput) {
        let _ = self.tx.try_send(AgentPanelCmd::ToolFeed(item));
    }

    /// Send a graceful shutdown signal to the TUI agent panel actor.
    ///
    /// The actor will exit its run loop after receiving this command.
    /// Side effect: silently drops the signal if the actor channel is full or stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(AgentPanelCmd::Shutdown);
    }
}
