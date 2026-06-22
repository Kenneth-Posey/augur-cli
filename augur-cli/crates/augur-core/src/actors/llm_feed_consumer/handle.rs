//! LlmFeedConsumerHandle: fire-and-forget client for the LLM feed consumer actor.

use super::llm_feed_consumer_ops::LlmFeedConsumerCmd;
use augur_domain::domain::types::StreamChunk;
use tokio::sync::mpsc;

/// Fire-and-forget handle to the running LLM feed consumer actor.
///
/// Cloning shares the same actor task. Callers send stream chunks for routing
/// without waiting for the route to complete. Dropping all clones causes the
/// actor's receiver to close.
#[derive(Clone)]
pub struct LlmFeedConsumerHandle {
    pub(crate) tx: mpsc::Sender<LlmFeedConsumerCmd>,
}

impl LlmFeedConsumerHandle {
    /// Create a new handle around the command sender. Called only by `spawn`.
    pub(super) fn new(tx: mpsc::Sender<LlmFeedConsumerCmd>) -> Self {
        LlmFeedConsumerHandle { tx }
    }

    /// Enqueue a stream chunk for classification and routing.
    ///
    /// Sends without blocking the caller. Silently drops the chunk if the
    /// actor channel is full or the actor has stopped.
    pub fn consume(&self, chunk: StreamChunk) {
        let _ = self.tx.try_send(LlmFeedConsumerCmd::Consume(chunk));
    }

    /// Send a graceful shutdown signal to the LLM feed consumer actor.
    ///
    /// The actor will exit its receive loop after processing this command.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(LlmFeedConsumerCmd::Shutdown);
    }
}
