//! Tool actor command types and ToolCall helper.

use crate::tools::handler::ToolCallResult;
use augur_domain::domain::types::StreamChunk;
use tokio::sync::oneshot;

pub use augur_domain::domain::types::ToolCall;

/// A request to execute a single tool call, with a oneshot reply channel.
///
/// The `reply_tx` is owned by this struct; the actor sends the result back
/// on it from `dispatch_tool_call`. There is no shared state: each execution
/// request has its own private reply channel.
pub struct ToolCallCommand {
    /// The name and arguments extracted from a `StreamChunk::ToolCall`.
    pub call: ToolCall,
    /// Oneshot sender for the tool's result; consumed by `dispatch_tool_call`.
    pub reply_tx: oneshot::Sender<ToolCallResult>,
}

/// Commands that flow through the tool actor's mpsc channel.
pub enum ToolCommand {
    /// Execute the given tool call and reply on the oneshot channel.
    Execute(ToolCallCommand),
    /// Gracefully stop the actor task loop.
    Shutdown,
}

/// Extract a `ToolCall` from a `StreamChunk::ToolCall` variant.
///
/// Returns `None` for all other variants. Pure function used by `AgentActor`
/// to identify tool calls in the LLM response stream without pattern-matching
/// the full enum at each call site.
pub fn build_tool_call(chunk: StreamChunk) -> Option<ToolCall> {
    match chunk {
        StreamChunk::ToolCall {
            id,
            name,
            arguments,
        } => Some(ToolCall {
            id,
            name,
            arguments,
        }),
        _ => None,
    }
}
