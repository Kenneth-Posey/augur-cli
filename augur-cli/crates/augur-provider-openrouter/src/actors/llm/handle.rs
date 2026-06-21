//! LlmHandle and the LlmClient trait for dependency-injected testing.
//!
//! `LlmClient` is defined in `domain::traits` and re-exported here so all
//! consumers can import from a single, stable path.

use augur_domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use augur_domain::types::StreamChunk;
use augur_domain::CompletionRequest;
use augur_provider_shared::request_context::LlmCommand;
use tokio::sync::mpsc;

pub use augur_domain::traits::LlmClient;

/// Cloneable handle to a running `LlmActor` task.
///
/// Wraps the command sender. Cheaply cloneable - all clones share the same
/// underlying channel to the actor. Use `complete_stream` to submit requests;
/// use `shutdown` to stop the actor on clean exit.
#[derive(Clone)]
pub struct LlmHandle {
    tx: mpsc::Sender<LlmCommand>,
}

impl LlmHandle {
    /// Create a handle from a raw command channel sender.
    ///
    /// Called only by `LlmActor::spawn` in `actor.rs`. Do not construct
    /// directly outside actor wiring.
    pub(super) fn new(tx: mpsc::Sender<LlmCommand>) -> Self {
        LlmHandle { tx }
    }

    /// Send a graceful shutdown signal to the actor.
    ///
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(LlmCommand::Shutdown);
    }

    /// Fire an automated user message at the LLM and return a reply receiver.
    ///
    /// Sends a `SendAutomated` command to the actor and returns the receive end
    /// of the reply channel. The actor passes the send end to the provider task,
    /// which streams `StreamChunk` events until `StreamChunk::Done` or
    /// `StreamChunk::Error`. Callers must consume or forward this receiver -
    /// dropping it silently discards the response. Uses `try_send`; on actor
    /// stop the reply channel is returned but will close immediately.
    pub fn send_automated(
        &self,
        text: OutputText,
        endpoint: EndpointName,
    ) -> mpsc::Receiver<StreamChunk> {
        let (reply_tx, reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
        let _ = self.tx.try_send(LlmCommand::SendAutomated {
            text,
            endpoint,
            reply_tx,
        });
        reply_rx
    }
}

impl LlmClient for LlmHandle {
    fn complete_stream(&self, request: CompletionRequest) -> mpsc::Receiver<StreamChunk> {
        let CompletionRequest {
            endpoint,
            messages,
            tools,
            cache,
            model_override,
        } = request;
        let (reply_tx, reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
        let error_tx = reply_tx.clone();
        let cmd = LlmCommand::Complete {
            endpoint,
            messages,
            tools,
            cache,
            reply_tx,
            model_override,
        };
        if let Err(e) = self.tx.try_send(cmd) {
            let msg = match &e {
                tokio::sync::mpsc::error::TrySendError::Full(_) => "LLM actor busy",
                tokio::sync::mpsc::error::TrySendError::Closed(_) => "LLM actor stopped",
            };
            let _ = error_tx.try_send(StreamChunk::Error(OutputText::new(msg)));
        }
        reply_rx
    }
}
