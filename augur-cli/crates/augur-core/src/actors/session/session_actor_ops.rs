//! Private helper operations for the session actor.

use super::session_ops::SessionCommand;
use augur_domain::domain::string_newtypes::EndpointName;
use tokio::sync::{mpsc, watch};

/// Actor task loop: processes endpoint-change and shutdown commands.
///
/// Exits on `SessionCommand::Shutdown` or when the command channel is closed.
pub(super) async fn run(
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    endpoint_tx: watch::Sender<EndpointName>,
) {
    loop {
        match cmd_rx.recv().await {
            None | Some(SessionCommand::Shutdown) => break,
            Some(SessionCommand::SetEndpoint(name)) => {
                let _ = endpoint_tx.send(name);
            }
        }
    }
}
