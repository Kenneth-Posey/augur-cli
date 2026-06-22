//! TuiMainFeedPanelHandle: fire-and-forget client for the TUI main feed panel actor.

use super::tui_main_feed_panel_ops::{MainFeedCmd, MainFeedState};
use augur_core::domain::deterministic_orchestrator::DeterministicOrchestratorEvent;
use augur_domain::domain::types::AgentOutput;
use tokio::sync::{mpsc, watch};

/// Fire-and-forget handle to the running TUI main feed panel actor.
///
/// Cloning shares the same underlying actor task. Callers push feed items
/// without blocking - the actor forwards them to the unified output channel.
/// Dropping all clones closes the actor's command channel.
#[derive(Clone)]
pub struct TuiMainFeedPanelHandle {
    tx: mpsc::Sender<MainFeedCmd>,
    state_rx: watch::Receiver<MainFeedState>,
}

impl TuiMainFeedPanelHandle {
    /// Create a handle from a command sender and state watch receiver. Called only by `spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<MainFeedCmd>,
        state_rx: watch::Receiver<MainFeedState>,
    ) -> Self {
        TuiMainFeedPanelHandle { tx, state_rx }
    }

    /// Return the current accumulated feed state by reading the watch-channel snapshot.
    ///
    /// This is a momentary borrow of the watch channel's internal cell - not
    /// shared mutable state. The value reflects whatever the actor last published.
    pub fn current_state(&self) -> MainFeedState {
        self.state_rx.borrow().clone()
    }

    /// Clone the watch receiver so the TUI runtime can subscribe to state updates.
    ///
    /// Returns a new `watch::Receiver<MainFeedState>` that tracks the same actor.
    pub fn state_rx(&self) -> watch::Receiver<MainFeedState> {
        self.state_rx.clone()
    }

    /// Forward a main agent output item to the unified feed channel.
    ///
    /// Inputs: `item` - the `AgentOutput` event from the main agent.
    /// Side effect: silently drops the item if the actor channel is full or stopped.
    pub fn send_agent(&self, item: AgentOutput) {
        let _ = self.tx.try_send(MainFeedCmd::Agent(item));
    }

    /// Forward an ask-panel output item to the unified feed channel.
    ///
    /// Inputs: `item` - the `AgentOutput` event from the ask panel.
    /// Side effect: silently drops the item if the actor channel is full or stopped.
    pub fn send_ask(&self, item: AgentOutput) {
        let _ = self.tx.try_send(MainFeedCmd::Ask(item));
    }

    /// Forward a deterministic orchestrator event to the unified feed channel.
    ///
    /// Inputs: `ev` - the `DeterministicOrchestratorEvent` to forward.
    /// Side effect: silently drops the event if the actor channel is full or stopped.
    pub fn send_orchestrator(&self, ev: DeterministicOrchestratorEvent) {
        let _ = self.tx.try_send(MainFeedCmd::Orchestrator(ev));
    }

    /// Send a graceful shutdown signal to the TUI main feed panel actor.
    ///
    /// The actor will exit its run loop after receiving this command.
    /// Side effect: silently drops the signal if the actor channel is full or stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(MainFeedCmd::Shutdown);
    }
}
