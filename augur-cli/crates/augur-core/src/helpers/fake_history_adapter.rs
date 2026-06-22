//! Test helper: factory for a throwaway `HistoryAdapterHandle` for use in tests.

use crate::actors::history_adapter::handle::HistoryAdapterHandle;
use crate::actors::history_adapter::history_adapter_actor::{HistoryAdapterConfig, spawn};

/// Spawn a minimal history-adapter actor and return its handle.
///
/// The downstream history-feed receiver is intentionally dropped, so any
/// recorded messages are silently discarded. Use in tests that construct
/// `AgentServices` or other structs requiring a `HistoryAdapterHandle`
/// without caring about actual history recording.
pub fn fake_history_adapter_handle() -> HistoryAdapterHandle {
    let (tx, _rx) = tokio::sync::mpsc::channel(16);
    let (_join, handle) = spawn(HistoryAdapterConfig {
        history_tx: tx,
        capacity: 16,
    });
    handle
}
