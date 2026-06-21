//! LLM feed consumer actor: classifies and routes `StreamChunk` items.

use super::handle::LlmFeedConsumerHandle;
use super::llm_feed_consumer_actor_ops as actor_ops;
use super::llm_feed_consumer_ops::LlmFeedOutputChannels;
use augur_domain::domain::channels::LLM_FEED_CAPACITY;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Spawn the LLM feed consumer actor and return its join handle and a communication handle.
///
/// Creates a bounded command channel using `LLM_FEED_CAPACITY`, wraps the
/// sender in a [`LlmFeedConsumerHandle`], and spawns the `run` loop as a
/// Tokio task. Callers send `StreamChunk` items via the handle; the actor
/// routes each to the matching output channel in `outputs`.
pub fn spawn(outputs: LlmFeedOutputChannels) -> (JoinHandle<()>, LlmFeedConsumerHandle) {
    let (tx, rx) = mpsc::channel(*LLM_FEED_CAPACITY);
    let handle = LlmFeedConsumerHandle::new(tx);
    let join = tokio::spawn(actor_ops::run(rx, outputs));
    (join, handle)
}
