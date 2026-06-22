//! Private helper operations for the TUI actor shell.

use super::handle::ShutdownSignal;
use super::tui_actor::TuiSpawnArgs;
use augur_domain::domain::types::FeedEntry;
use tokio::sync::{mpsc, watch};

/// Spawn the TUI runtime task.
pub(super) fn spawn_run(
    args: TuiSpawnArgs,
    shutdown_tx: watch::Sender<ShutdownSignal>,
    feed_rx: mpsc::Receiver<FeedEntry>,
) -> tokio::task::JoinHandle<()> {
    super::tui_actor::spawn_runtime_task(args, shutdown_tx, feed_rx)
}
