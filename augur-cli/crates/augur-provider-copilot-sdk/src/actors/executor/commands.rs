//! Executor actor command types and local session event representation.
//!
//! `ExecutorCmd` is the inbound command enum for the actor's command loop.
//! `SessionEvent` is a local mirror of the CLI session event stream - the
//! actor translates SDK-specific types into these before calling `event_mapper`,
//! keeping `event_mapper` free of SDK dependencies.

use augur_domain::newtypes::TokenCount;
use augur_domain::plan_tree::PlanNodeId;
use augur_domain::string_newtypes::{
    OutputText, ProcessId, PromptText, ShellCommand, ToolCallId, ToolName,
};
use augur_domain::traits::ExecutorMode;
use tokio::sync::oneshot;

/// Result of a shell command executed through the CLI session.
///
/// Mirrors the SDK result shape. The SDK returns a `process_id` when the
/// shell command is submitted; stdout and exit code arrive asynchronously
/// through the session event stream.
#[derive(Clone, Debug)]
pub struct ShellExecResult {
    /// Process identifier assigned by the SDK to the shell command.
    pub process_id: ProcessId,
}

/// Inbound commands for `ExecutorActor`.
///
/// Sent through the actor's `mpsc` command channel by `ExecutorHandle`.
/// The actor dispatches each variant to the underlying CLI session.
#[derive(Debug)]
pub enum ExecutorCmd {
    /// Send a plain-text prompt to the CLI session.
    SendPrompt { content: PromptText },
    /// Switch the session into the given operational mode.
    SetMode { mode: ExecutorMode },
    /// Trigger conversation compaction on the session.
    Compact,
    /// Execute a shell command through the session and return the result.
    ShellExec {
        /// The shell command to run.
        command: ShellCommand,
        /// Channel for returning the result to the caller.
        reply_tx: oneshot::Sender<ShellExecResult>,
    },
    /// Gracefully stop the actor and disconnect the session.
    Stop,
}

/// Local mirror of the CLI session event stream.
///
/// The actor converts SDK `SessionEventData` values into this enum before
/// calling `event_mapper::map_session_event`. This keeps `event_mapper` free of
/// SDK types and fully testable without the `copilot-executor` feature flag.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionEvent {
    /// A partial assistant text token arrived in the stream.
    AssistantMessageDelta {
        /// The incremental text content of the delta.
        content: OutputText,
    },
    /// The assistant completed a full message (turn-level signal).
    AssistantMessageComplete,
    /// A tool execution started.
    ToolExecutionStart {
        /// Name of the tool being executed.
        tool_name: ToolName,
        /// Arguments passed to the tool, if any.
        args: serde_json::Value,
    },
    /// A tool execution completed.
    ToolExecutionComplete {
        /// SDK-assigned identifier for the completed tool call.
        tool_call_id: ToolCallId,
    },
    /// The session encountered an error.
    SessionError {
        /// Human-readable error description.
        message: String,
    },
    /// The session is idle and ready for the next prompt.
    ///
    /// Used by the supervisor to advance to the next plan step.
    SessionIdle,
    /// The `update_plan_step` tool was called by the CLI agent.
    ///
    /// Carries the parsed tool arguments so `event_mapper` can produce
    /// an `AgentOutput::PlanNodeUpdate` without re-parsing JSON.
    PlanNodeUpdated {
        /// The node id string as provided in the tool call.
        node_id: PlanNodeId,
        /// Status string: `"in_progress"`, `"done"`, or `"failed"`.
        status: String,
        /// Optional notes or failure reason.
        notes: Option<String>,
    },
    /// Token usage reported by the assistant for the completed turn.
    ///
    /// Carries optional input, output, and cache-read token counts from the SDK's
    /// `AssistantUsage` event. Any field may be absent when the SDK omits it.
    AssistantUsage {
        /// Number of input (prompt) tokens consumed, when reported.
        input_tokens: Option<TokenCount>,
        /// Number of output (completion) tokens produced, when reported.
        output_tokens: Option<TokenCount>,
        /// Number of cached input tokens served from the provider cache, when reported.
        cache_read_tokens: Option<TokenCount>,
    },
    /// Any SDK event not mapped to a known variant.
    Unknown,
    /// The model stated its intent before executing tool calls (AssistantIntent).
    ///
    /// Emitted when the SDK fires an `AssistantIntent` event. Carries the intent
    /// string so `event_mapper` can produce `AgentOutput::IntentMessage`.
    AssistantIntent {
        /// The model's stated intent text.
        intent: OutputText,
    },
    /// A live progress update from a running tool execution (ToolExecutionProgress).
    ///
    /// Carries the SDK-assigned `tool_call_id` for future correlation and a
    /// human-readable progress message.
    ToolProgress {
        /// SDK-assigned identifier for the tool call that produced this update.
        tool_call_id: ToolCallId,
        /// Human-readable progress description from the tool.
        message: OutputText,
    },
    /// A streaming partial output chunk from a running tool execution (ToolExecutionPartialResult).
    ///
    /// Carries the SDK-assigned `tool_call_id` and a partial output text chunk,
    /// which may contain newlines.
    ToolPartialResult {
        /// SDK-assigned identifier for the tool call that produced this chunk.
        tool_call_id: ToolCallId,
        /// Partial output text, which may contain newlines.
        output: OutputText,
    },
}
