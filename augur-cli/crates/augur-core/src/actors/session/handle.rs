//! SessionHandle: public interface for reading and changing the active endpoint.

use super::session_ops::SessionCommand;
use augur_domain::domain::string_newtypes::{EndpointName, ModelId};
use augur_domain::domain::thinking_mode::ReasoningEffort;
use tokio::sync::{mpsc, watch};

/// Handle to a running `SessionActor` task.
///
/// Provides a watch-channel snapshot of the currently selected endpoint and
/// a command sender for endpoint changes. No shared mutable state - endpoint
/// reads are watch channel borrows and writes are mpsc sends.
#[derive(Clone)]
pub struct SessionHandle {
    tx: mpsc::Sender<SessionCommand>,
    endpoint_rx: watch::Receiver<EndpointName>,
}

impl SessionHandle {
    /// Create a handle. Called only by `SessionActor::spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<SessionCommand>,
        endpoint_rx: watch::Receiver<EndpointName>,
    ) -> Self {
        SessionHandle { tx, endpoint_rx }
    }

    /// Return the current active endpoint by reading the watch channel snapshot.
    ///
    /// This is a momentary borrow of the watch channel's internal cell - not
    /// shared mutable state. The value reflects whatever the actor last set.
    pub fn active_endpoint(&self) -> EndpointName {
        self.endpoint_rx.borrow().clone()
    }

    /// Request a change to the active endpoint.
    ///
    /// Returns `Ok(())` when the request was enqueued successfully.
    pub async fn set_endpoint(&self, name: EndpointName) -> anyhow::Result<()> {
        self.tx
            .send(SessionCommand::SetEndpoint(name))
            .await
            .map_err(|_| anyhow::anyhow!("session actor queue unavailable"))
    }

    /// Persist user-facing endpoint/model/reasoning settings.
    ///
    /// This is the facade boundary for UI-triggered settings writes. Callers
    /// should use this method instead of writing config files directly.
    ///
    /// `endpoint`: selected endpoint, or `None` to clear.
    /// `model`: selected model override, or `None` for endpoint default/auto.
    /// `effort`: selected reasoning effort, or `None` when not applicable.
    pub fn save_user_settings(
        &self,
        endpoint: Option<&EndpointName>,
        model: Option<&ModelId>,
        effort: Option<&ReasoningEffort>,
    ) {
        crate::config::user_settings::save_user_settings(endpoint, model, effort);
    }

    /// Send a graceful shutdown signal to the session actor.
    ///
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(SessionCommand::Shutdown);
    }
}
