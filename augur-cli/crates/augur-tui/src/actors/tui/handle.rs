//! TuiHandle: signals to the caller that the TUI has exited.

use augur_domain::domain::types::FeedEntry;
use tokio::sync::{mpsc, watch};

/// Lifecycle signal broadcast on the TUI shutdown watch channel.
///
/// Sent by the TUI actor loop to notify `TuiHandle::wait_for_shutdown`
/// whether the actor is still active or has completed its run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShutdownSignal {
    /// The TUI actor is still running; no action needed.
    Running,
    /// The TUI actor has exited; the caller may now terminate.
    Complete,
}

/// Handle to a running `TuiActor` task.
///
/// Provides `wait_for_shutdown` to block until the TUI exits, and
/// `agent_feed_tx` to push `FeedEntry` events into the agent feed panel.
pub struct TuiHandle {
    shutdown_rx: watch::Receiver<ShutdownSignal>,
    /// Sender half of the agent feed channel. Clone and share with background tasks.
    pub agent_feed_tx: mpsc::Sender<FeedEntry>,
}

impl TuiHandle {
    /// Create a handle from a watch receiver and an agent feed sender.
    ///
    /// Called by `TuiActor::spawn`. In tests that only exercise `wait_for_shutdown`,
    /// create a dummy channel: `let (tx, _) = tokio::sync::mpsc::channel(1);`.
    pub(crate) fn new(
        shutdown_rx: watch::Receiver<ShutdownSignal>,
        agent_feed_tx: mpsc::Sender<FeedEntry>,
    ) -> Self {
        TuiHandle {
            shutdown_rx,
            agent_feed_tx,
        }
    }

    /// Block until the TUI signals shutdown (the watch channel becomes `true`).
    ///
    /// Called by `main` to keep the process alive while the TUI runs.
    /// Returns as soon as the actor loop exits and sends `true` on the channel.
    #[tracing::instrument(skip(self))]
    pub async fn wait_for_shutdown(&mut self) {
        loop {
            if matches!(*self.shutdown_rx.borrow(), ShutdownSignal::Complete) {
                break;
            }
            if self.shutdown_rx.changed().await.is_err() {
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/actors/tui/handle.tests.rs"]
mod tests;
