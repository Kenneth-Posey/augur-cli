//! Private helper operations for the history-adapter actor.

use super::history_adapter_ops::{HistoryAdapterCmd, to_history_entry};
use augur_domain::domain::feeds::HistoryFeedMessage;
use tokio::sync::mpsc;

/// Actor receive loop: converts each command to a feed entry and exits on `Shutdown`.
///
/// Inputs: `rx` - command receiver; `history_tx` - downstream history feed sender.
/// Side effect: each `RecordUser` or `RecordLlm` command is converted via
/// `to_history_entry` and sent to `history_tx` (send errors are silently ignored).
/// The loop exits when `Shutdown` is received or the sender is dropped.
pub(super) async fn run(
    mut rx: mpsc::Receiver<HistoryAdapterCmd>,
    history_tx: mpsc::Sender<HistoryFeedMessage>,
) {
    while let Some(cmd) = rx.recv().await {
        match to_history_entry(&cmd) {
            Some(entry) => {
                let _ = history_tx.try_send(entry);
            }
            None => break,
        }
    }
}
