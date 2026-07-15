//! `SupervisorHandle` - cloneable handle to a running `SupervisorActor`.
//!
//! Exposes command sending and event subscription. Only `wiring.rs`
//! constructs this handle.

use super::commands::SupervisorCmd;
use augur_domain::domain::channels::SUPERVISOR_OUTPUT_CAPACITY;
use augur_domain::domain::types::SupervisorEvent;
use tokio::sync::{broadcast, mpsc};

/// Cloneable handle to a running `SupervisorActor`.
#[derive(Clone)]
pub struct SupervisorHandle {
    cmd_tx: mpsc::Sender<SupervisorCmd>,
    event_tx: broadcast::Sender<SupervisorEvent>,
}

impl SupervisorHandle {
    /// Construct a handle from raw channel endpoints.
    ///
    /// Called only by `SupervisorActor::spawn`.
    pub(super) fn new(
        cmd_tx: mpsc::Sender<SupervisorCmd>,
        event_tx: broadcast::Sender<SupervisorEvent>,
    ) -> Self {
        SupervisorHandle { cmd_tx, event_tx }
    }

    /// Subscribe to the supervisor event broadcast channel.
    pub fn subscribe_events(&self) -> broadcast::Receiver<SupervisorEvent> {
        self.event_tx.subscribe()
    }

    /// Send a graceful stop signal to the actor.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(SupervisorCmd::Stop);
    }
}

/// Create a broadcast sender for the supervisor event channel.
///
/// Called by `SupervisorActor::spawn`. The sender is stored in the handle;
/// subscribers call `subscribe_events` on the handle.
pub(super) fn make_event_channel() -> broadcast::Sender<SupervisorEvent> {
    let (tx, _) = broadcast::channel(*SUPERVISOR_OUTPUT_CAPACITY);
    tx
}
