//! LLM feed consumer ops: pure chunk classification and output routing.
//!
//! `classify_chunk` maps a `StreamChunk` variant to a `LlmFeedTag`.
//! `route_chunk` classifies and dispatches to the correct output channel.

use augur_domain::domain::feeds::{LlmFeedMessage, LlmFeedTag};
use augur_domain::domain::types::StreamChunk;
use tokio::sync::mpsc;

// ── LlmFeedConsumerCmd ────────────────────────────────────────────────────────

/// Commands accepted by the LLM feed consumer actor.
///
/// `Consume` delivers a stream chunk for routing. `Shutdown` signals the actor
/// to exit its receive loop cleanly.
#[derive(Debug)]
pub enum LlmFeedConsumerCmd {
    /// Deliver a stream chunk to be classified and routed.
    Consume(StreamChunk),
    /// Signal the actor to exit its receive loop.
    Shutdown,
}

// ── LlmFeedOutputChannels ────────────────────────────────────────────────────

/// Bundle of output sender channels for the four routable feed categories.
///
/// Constructed with `LlmFeedOutputChannels::builder()`. Each field is a
/// `mpsc::Sender<LlmFeedMessage>` for one of the four routable `LlmFeedTag`
/// categories. The actor holds one instance and routes every inbound chunk
/// to exactly one channel.
#[derive(bon::Builder)]
pub struct LlmFeedOutputChannels {
    /// Sender for chunks classified as [`LlmFeedTag::BackgroundAgentChunk`].
    pub bg_agent_tx: mpsc::Sender<LlmFeedMessage>,
    /// Sender for chunks classified as [`LlmFeedTag::ThinkingChatter`].
    pub thinking_tx: mpsc::Sender<LlmFeedMessage>,
    /// Sender for chunks classified as [`LlmFeedTag::UserChunk`] or [`LlmFeedTag::Error`].
    pub user_chunk_tx: mpsc::Sender<LlmFeedMessage>,
    /// Sender for chunks classified as [`LlmFeedTag::ToolRequest`].
    pub tool_request_tx: mpsc::Sender<LlmFeedMessage>,
}

// ── classify_chunk ────────────────────────────────────────────────────────────

/// Map a `StreamChunk` variant to its semantic `LlmFeedTag`.
///
/// Inputs: reference to the chunk to classify.
/// Outputs: the `LlmFeedTag` for routing decisions.
/// `Token` → `UserChunk`. `ToolCall` → `ToolRequest`. `Error` → `Error`.
/// Control signals (`Done`, `Usage`, `RateLimitRetry`) pass through as `UserChunk`.
pub fn classify_chunk(chunk: &StreamChunk) -> LlmFeedTag {
    match chunk {
        StreamChunk::Token(_) => LlmFeedTag::UserChunk,
        StreamChunk::ToolCall { .. } => LlmFeedTag::ToolRequest,
        StreamChunk::Error(_) => LlmFeedTag::Error,
        _ => LlmFeedTag::UserChunk,
    }
}

// ── route_chunk ───────────────────────────────────────────────────────────────

/// Classify a chunk and dispatch it to the matching output channel.
///
/// Inputs: `chunk` - the stream chunk to route; `outputs` - output channel bundle.
/// Side effect: sends to one of the four output channels via `try_send`.
/// Back-pressure is intentionally ignored: a full or closed receiver silently
/// drops the message so the actor loop is never blocked.
pub fn route_chunk(chunk: StreamChunk, outputs: &LlmFeedOutputChannels) {
    let tag = classify_chunk(&chunk);
    let msg = LlmFeedMessage {
        tag: tag.clone(),
        chunk,
    };
    let result = match tag {
        LlmFeedTag::BackgroundAgentChunk => outputs.bg_agent_tx.try_send(msg),
        LlmFeedTag::ThinkingChatter => outputs.thinking_tx.try_send(msg),
        LlmFeedTag::ToolRequest => outputs.tool_request_tx.try_send(msg),
        LlmFeedTag::UserChunk | LlmFeedTag::Error => outputs.user_chunk_tx.try_send(msg),
    };
    let _ = result; // intentionally ignore back-pressure
}
