//! Private helper operations for the active-model actor.

use super::active_model_ops::ActiveModelCommand;
use augur_domain::domain::string_newtypes::ModelId;
use tokio::sync::{mpsc, watch};

/// Actor task loop: receives `Set` commands and forwards to the watch sender.
///
/// Exits when the command channel is closed (i.e., all `ActiveModelHandle`
/// clones that hold a `tx` have been dropped).
pub(super) async fn run(
    mut cmd_rx: mpsc::Receiver<ActiveModelCommand>,
    model_tx: watch::Sender<Option<ModelId>>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            ActiveModelCommand::Set(model_id) => {
                let _ = model_tx.send(Some(model_id));
            }
        }
    }
}
