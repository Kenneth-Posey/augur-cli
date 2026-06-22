//! Private helper operations for the LLM-feed consumer actor.

use super::llm_feed_consumer_ops::{LlmFeedConsumerCmd, LlmFeedOutputChannels, route_chunk};
use tokio::sync::mpsc;

/// Actor receive loop: routes each `Consume` command and exits on `Shutdown`.
///
/// Inputs: `rx` - command receiver; `outputs` - output channel bundle.
/// Side effect: each `Consume(chunk)` is classified and dispatched via `route_chunk`.
pub(super) async fn run(
    mut rx: mpsc::Receiver<LlmFeedConsumerCmd>,
    outputs: LlmFeedOutputChannels,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            LlmFeedConsumerCmd::Consume(chunk) => route_chunk(chunk, &outputs),
            LlmFeedConsumerCmd::Shutdown => break,
        }
    }
}
