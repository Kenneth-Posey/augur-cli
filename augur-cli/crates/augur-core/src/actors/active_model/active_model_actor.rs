//! Active-model actor: stores the currently active model and publishes it via watch.

use super::active_model_actor_ops as actor_ops;
use super::active_model_ops::ActiveModelCommand;
use super::handle::ActiveModelHandle;
use augur_domain::domain::string_newtypes::ModelId;
use tokio::sync::{mpsc, watch};

/// Spawn the active-model actor and return a handle.
///
/// Creates a `watch::channel` seeded with `None` (no model selected yet) and
/// an `mpsc::channel` for `Set` commands. The actor task owns the
/// `watch::Sender`; callers read the current model through `ActiveModelHandle`.
///
/// # Returns
///
/// An `ActiveModelHandle` that can be cloned freely. The actor exits when all
/// senders are dropped and the mpsc channel closes.
pub fn spawn() -> ActiveModelHandle {
    let (model_tx, model_rx) = watch::channel::<Option<ModelId>>(None);
    let (cmd_tx, cmd_rx) = mpsc::channel::<ActiveModelCommand>(8);
    let handle = ActiveModelHandle::new(cmd_tx, model_rx);
    tokio::spawn(actor_ops::run(cmd_rx, model_tx));
    handle
}
