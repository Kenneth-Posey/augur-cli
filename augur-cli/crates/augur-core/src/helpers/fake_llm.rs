//! FakeLlmClient: pre-loaded streaming responses for use in agent actor tests.

use augur_domain::domain::traits::CompletionRequest;
use augur_domain::domain::traits::LlmClient;
use augur_domain::domain::types::{Message, StreamChunk};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// A test double for `LlmClient` that returns pre-loaded response sequences.
///
/// Constructed with a list of response batches; each call to `complete_stream`
/// pops the next batch and sends its chunks. Also records every `messages`
/// argument received so tests can assert conversation history contents.
/// Cloning shares the internal `Arc` state, allowing the clone to be moved
/// into `AgentActor::spawn` while the original retains read access.
pub struct FakeLlmClient {
    responses: Arc<Mutex<VecDeque<Vec<StreamChunk>>>>,
    /// All message lists received by `complete_stream`, in call order.
    pub received: Arc<Mutex<Vec<Vec<Message>>>>,
}

impl FakeLlmClient {
    /// Create a new fake with the given ordered response batches.
    ///
    /// Each inner `Vec<StreamChunk>` is returned as one stream response.
    /// If a call arrives after all batches are exhausted, an empty batch
    /// is returned (channel closes immediately, treated as `Done`).
    pub fn new(responses: Vec<Vec<StreamChunk>>) -> Self {
        FakeLlmClient {
            responses: Arc::new(Mutex::new(responses.into())),
            received: Arc::new(Mutex::new(vec![])),
        }
    }
}

impl Clone for FakeLlmClient {
    fn clone(&self) -> Self {
        FakeLlmClient {
            responses: Arc::clone(&self.responses),
            received: Arc::clone(&self.received),
        }
    }
}

impl LlmClient for FakeLlmClient {
    fn complete_stream(&self, request: CompletionRequest) -> mpsc::Receiver<StreamChunk> {
        let CompletionRequest { messages, .. } = request;
        self.received.lock().unwrap().push(messages);
        let chunks = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default();
        let (tx, rx) = mpsc::channel(chunks.len().max(1));
        tokio::spawn(async move {
            for chunk in chunks {
                let _ = tx.send(chunk).await;
            }
        });
        rx
    }
}
