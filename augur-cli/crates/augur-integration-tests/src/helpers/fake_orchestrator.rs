//! Fake `DeterministicOrchestratorHandle` for use in TUI unit tests.

use crate::actors::DeterministicOrchestratorHandle;
use tokio::sync::{broadcast, mpsc};

/// Builds a disconnected `DeterministicOrchestratorHandle` whose command
/// channel is never read.  Tests that construct `TuiHandles` directly need
/// an orchestrator field; this satisfies that requirement without spawning a
/// real actor.
pub fn fake_orchestrator_handle() -> DeterministicOrchestratorHandle {
    let (cmd_tx, _cmd_rx) = mpsc::channel(1);
    let (event_tx, _event_rx) = broadcast::channel(1);
    let (auto_msg_tx, _auto_msg_rx) = broadcast::channel(1);
    DeterministicOrchestratorHandle::new(cmd_tx, event_tx, auto_msg_tx)
}
