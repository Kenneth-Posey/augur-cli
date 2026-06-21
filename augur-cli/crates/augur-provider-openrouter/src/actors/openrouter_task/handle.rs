//! `OpenRouterTaskHandle`: the public interface for sending spawn requests to
//! a running `OpenRouterTaskActor` task.

use augur_domain::task_types::SpawnAgentRequest;
use tokio::sync::mpsc;

/// Cloneable handle to a running `OpenRouterTaskActor` task.
///
/// Wraps the spawn-request sender so that external callers (e.g.
/// `EndpointRoutingChatProvider`) can submit additional sub-agent spawn requests
/// without direct access to the actor internals. Returned by
/// `OpenRouterTaskActor::spawn(args)` alongside the `JoinHandle`.
#[derive(Clone, Debug)]
pub struct OpenRouterTaskHandle {
    #[allow(dead_code)]
    pub(crate) spawn_tx: mpsc::Sender<SpawnAgentRequest>,
}

impl OpenRouterTaskHandle {
    /// Wrap a spawn-request sender. Called only by `OpenRouterTaskActor::spawn`.
    pub(crate) fn new(spawn_tx: mpsc::Sender<SpawnAgentRequest>) -> Self {
        Self { spawn_tx }
    }
}
