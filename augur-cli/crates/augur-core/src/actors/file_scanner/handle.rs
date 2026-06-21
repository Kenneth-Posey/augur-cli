//! FileScannerHandle: non-blocking client for the file-scanner actor.

use super::commands::FileScanCmd;
use augur_domain::domain::string_newtypes::FilePath;
use augur_domain::domain::types::FileCompletion;
use tokio::sync::{mpsc, watch};

/// Cloneable client handle to the running file-scanner actor.
///
/// The TUI holds one instance and calls `scan(prefix)` on each keypress after
/// `@`. Results are published to a watch channel and retrieved non-blocking
/// via `latest()` - the TUI event loop never awaits a scan response.
#[derive(Clone)]
pub struct FileScannerHandle {
    cmd_tx: mpsc::Sender<FileScanCmd>,
    results_rx: watch::Receiver<Vec<FileCompletion>>,
}

impl FileScannerHandle {
    /// Create a new handle. Called only by `actor::spawn`.
    pub(super) fn new(
        cmd_tx: mpsc::Sender<FileScanCmd>,
        results_rx: watch::Receiver<Vec<FileCompletion>>,
    ) -> Self {
        FileScannerHandle { cmd_tx, results_rx }
    }

    /// Queue a directory scan for paths matching `prefix`.
    ///
    /// Non-blocking: sends the command via `try_send` and returns immediately.
    /// If the actor is shut down or the channel is full, the command is dropped
    /// silently - the TUI will retry on the next keypress.
    pub fn scan(&self, prefix: impl Into<FilePath>) {
        let _ = self.cmd_tx.try_send(FileScanCmd::Scan {
            prefix: prefix.into(),
        });
    }

    /// Return the most recently published scan results without blocking.
    ///
    /// Borrows the current watch value and clones it. Returns an empty vec
    /// before the first scan completes or when the actor is shut down.
    pub fn latest(&self) -> Vec<FileCompletion> {
        self.results_rx.borrow().clone()
    }

    /// Send a graceful shutdown signal to the file-scanner actor.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(FileScanCmd::Shutdown);
    }
}
