//! Session actor: owns the active endpoint selection and publishes it via watch.

use super::handle::SessionHandle;
use super::session_actor_ops as actor_ops;
use augur_domain::domain::channels::SESSION_COMMAND_CAPACITY;
use augur_domain::domain::string_newtypes::EndpointName;
use tokio::sync::{mpsc, watch};

/// Spawn the session actor and return a join handle plus a `SessionHandle`.
///
/// Creates a `watch::channel` seeded with `default`, which becomes the initial
/// active endpoint. Creates an `mpsc::channel` for commands. The actor task
/// owns the `watch::Sender`; callers read snapshots via `SessionHandle`.
#[tracing::instrument(level = "info", fields(default = %default))]
pub fn spawn(default: EndpointName) -> (tokio::task::JoinHandle<()>, SessionHandle) {
    let (endpoint_tx, endpoint_rx) = watch::channel(default);
    let (cmd_tx, cmd_rx) = mpsc::channel(*SESSION_COMMAND_CAPACITY);
    let handle = SessionHandle::new(cmd_tx, endpoint_rx);
    let join = tokio::spawn(actor_ops::run(cmd_rx, endpoint_tx));
    (join, handle)
}
