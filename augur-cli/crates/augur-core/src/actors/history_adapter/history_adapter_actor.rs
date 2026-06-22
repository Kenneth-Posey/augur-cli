//! History adapter actor: consumes `HistoryAdapterCmd` items and re-emits typed
//! `HistoryFeedMessage` items to the logger's history input channel.

use super::handle::HistoryAdapterHandle;
use super::history_adapter_actor_ops as actor_ops;
use augur_domain::domain::feeds::HistoryFeedMessage;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

// ── HistoryAdapterConfig ──────────────────────────────────────────────────────

/// Configuration for spawning the history adapter actor.
///
/// `history_tx` is the sender end of the history feed channel - the actor
/// forwards every classified entry to this channel. `capacity` is the bound
/// for the actor's own command channel (typically `HISTORY_FEED_CAPACITY.inner()`).
pub struct HistoryAdapterConfig {
    /// Sender for the downstream history feed channel.
    pub history_tx: mpsc::Sender<HistoryFeedMessage>,
    /// Capacity of the actor's internal command channel.
    pub capacity: usize,
}

// ── spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the history adapter actor and return its join handle and a communication handle.
///
/// Creates a bounded command channel using `config.capacity`, wraps the sender
/// in a [`HistoryAdapterHandle`], and spawns the `run` loop as a Tokio task.
/// Callers send user or LLM messages via the handle; the actor classifies each
/// and forwards it as a typed [`HistoryFeedMessage`] to `config.history_tx`.
pub fn spawn(config: HistoryAdapterConfig) -> (JoinHandle<()>, HistoryAdapterHandle) {
    let (tx, rx) = mpsc::channel(config.capacity);
    let handle = HistoryAdapterHandle::new(tx);
    let join = tokio::spawn(actor_ops::run(rx, config.history_tx));
    (join, handle)
}
