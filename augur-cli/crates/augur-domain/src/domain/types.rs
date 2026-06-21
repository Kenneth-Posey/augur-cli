//! Core message and stream types shared across actors.

use crate::domain::newtypes::{
    Count, ExecutionSuccess, Temperature, TimestampMs, TokenCount, ToolResultStripFraction,
    UsdCost, WaitSecs,
};
use crate::domain::plan_tree::{CheckpointConfig, PlanNodeId, PlanTree};
use crate::domain::string_newtypes::{
    AgentName, CachedFileContent, EndpointName, FailureReason, FileDisplayName, FilePath, ModelId,
    ModelLabel, OutputText, PromptText, StatusLabel, StringNewtype, ToolCallId, ToolName,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Semantic state for an agent-turn cancellation signal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CancelSignal {
    /// No cancellation has been requested.
    #[default]
    Clear,
    /// The current turn should stop as soon as the receiver observes the signal.
    Cancelled,
}

impl From<FailureReason> for OutputText {
    fn from(value: FailureReason) -> Self {
        OutputText::from(value.as_str())
    }
}

/// Whether a Copilot SDK session is still alive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SessionAliveness {
    /// The session is still valid and can continue processing requests.
    #[default]
    Alive,
    /// The session is dead and must be recreated or resumed.
    Dead,
}

/// The role a message plays in a conversation.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

/// A single conversation message with role, content, and a creation timestamp.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: OutputText,
    pub timestamp: TimestampMs,
    /// Provider-assigned tool call ID for `Role::Tool` messages.
    ///
    /// Set from the originating `ToolCall::id` so that the OpenAI-compatible
    /// provider can emit `"tool_call_id"` on the tool result message. `None`
    /// for all non-tool roles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<ToolCallId>,
    /// Tool calls included in this assistant message.
    ///
    /// Present only on `Role::Assistant` messages that triggered tool
    /// execution. The OpenAI-compatible provider uses this to emit the
    /// `"tool_calls"` array so providers can correlate tool results. `None`
    /// for all messages without tool calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl Message {
    /// Create a user message from a prompt. Timestamps with `TimestampMs::now()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use augur_core::domain::types::Message;
    /// let msg = Message::user("What is 2 + 2?");
    /// // Message is successfully created
    /// ```
    ///
    /// # See also
    ///
    /// - [`Message::assistant`] - Create assistant response messages
    /// - [`Message::system`] - Create system prompt messages
    /// - [`Message::tool_result`] - Create tool execution result messages
    pub fn user(text: impl Into<PromptText>) -> Self {
        let content = OutputText::new(text.into().into_inner());
        Message {
            role: Role::User,
            content,
            timestamp: TimestampMs::now(),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Create an assistant message. Timestamps with `TimestampMs::now()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use augur_core::domain::types::Message;
    /// let msg = Message::assistant("The answer is 4.");
    /// // Message is successfully created
    /// ```
    ///
    /// # See also
    ///
    /// - [`Message::user`] - Create user input messages
    /// - [`Message::system`] - Create system prompt messages
    pub fn assistant(text: impl Into<OutputText>) -> Self {
        Message {
            role: Role::Assistant,
            content: text.into(),
            timestamp: TimestampMs::now(),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Create a system prompt message. Timestamps with `TimestampMs::now()`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use augur_core::domain::types::Message;
    /// let msg = Message::system("You are a helpful coding assistant.");
    /// // Message is successfully created
    /// ```
    ///
    /// # See also
    ///
    /// - [`Message::user`] - Create user input messages
    /// - [`Message::assistant`] - Create assistant response messages
    pub fn system(text: impl Into<OutputText>) -> Self {
        Message {
            role: Role::System,
            content: text.into(),
            timestamp: TimestampMs::now(),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    /// Create a tool-result message, prefixed with `"[{name}]: "`.
    ///
    /// Stores the `tool_call_id` so that OpenAI-compatible providers can emit
    /// `"tool_call_id"` on the corresponding tool result message. This is the
    /// single formatting site for tool result messages; the prefix lets the LLM
    /// identify which tool produced the output. Called by the agent actor after
    /// each tool execution.
    pub fn tool_result(
        tool_call_id: ToolCallId,
        name: &ToolName,
        text: impl Into<OutputText>,
    ) -> Self {
        let prefixed = format!("[{}]: {}", name.as_str(), text.into().as_str());
        Message {
            role: Role::Tool,
            content: OutputText::new(prefixed),
            timestamp: TimestampMs::now(),
            tool_call_id: Some(tool_call_id),
            tool_calls: None,
        }
    }

    /// Create an assistant message that carries the tool calls it requested.
    ///
    /// Used when the LLM response included tool calls. The call list is stored
    /// in `tool_calls` so that OpenAI-compatible providers can reconstruct the
    /// `"tool_calls"` array in the assistant message when building the request
    /// body for the next turn. `text` may be empty for pure tool-call responses.
    pub fn assistant_with_tool_calls(text: impl Into<OutputText>, calls: Vec<ToolCall>) -> Self {
        Message {
            role: Role::Assistant,
            content: text.into(),
            timestamp: TimestampMs::now(),
            tool_call_id: None,
            tool_calls: Some(calls),
        }
    }
}

/// Token and cost counts from a single LLM turn.
///
/// Grouped here so [`LlmUsage`] stays within the 5-field limit.
/// All fields are non-negative; `cache_write_tokens` and `cost_usd` default to
/// zero when the provider does not report them.
#[derive(Clone, Debug, Default, PartialEq, bon::Builder, serde::Serialize, serde::Deserialize)]
pub struct LlmTokenCounts {
    /// Prompt (input) token count from the provider response.
    pub tokens_in: TokenCount,
    /// Completion (output) token count from the provider response.
    pub tokens_out: TokenCount,
    /// Cached input tokens (Anthropic: `cache_read_input_tokens`; OpenAI: 0).
    pub tokens_cached: TokenCount,
    /// Cache-write tokens (Anthropic: `cache_creation_input_tokens`; OpenAI: 0).
    ///
    /// Defaults to zero when the provider does not report cache writes.
    #[serde(default)]
    #[builder(default)]
    pub cache_write_tokens: TokenCount,
    /// Dollar cost of this turn as reported by the SDK (`AssistantUsageData.cost`).
    ///
    /// Defaults to `0.0` when the SDK omits the cost field. Always non-negative.
    #[serde(default)]
    #[builder(default)]
    pub cost_usd: UsdCost,
}

/// LLM generation metadata captured from a completed streaming response.
///
/// Emitted as `StreamChunk::Usage` after the last token/tool-call chunk and
/// before `StreamChunk::Done`. The agent actor captures this chunk and attaches
/// it to the final assistant `MessageRecord` when persisting the session.
/// `temperature` is the request parameter from `GenerationParams` - not from
/// the response body.
///
/// Token and cost fields are accessible directly via `Deref<Target = LlmTokenCounts>`.
#[derive(Clone, Debug, PartialEq, bon::Builder, serde::Serialize, serde::Deserialize)]
pub struct LlmUsage {
    /// Model name from the response body (e.g. `"claude-opus-4-6"`).
    pub model: OutputText,
    /// Token and cost counts for this turn.
    #[serde(flatten)]
    pub token_counts: LlmTokenCounts,
    /// Sampling temperature that was sent in the request.
    pub temperature: Temperature,
}

impl std::ops::Deref for LlmUsage {
    type Target = LlmTokenCounts;
    fn deref(&self) -> &LlmTokenCounts {
        &self.token_counts
    }
}

impl std::ops::DerefMut for LlmUsage {
    fn deref_mut(&mut self) -> &mut LlmTokenCounts {
        &mut self.token_counts
    }
}

/// Accumulated token and cost totals across all LLM turns in a session.
///
/// Owned exclusively by `TokenTrackerActor` in memory for the current process.
/// All fields are monotonically non-decreasing within a session (only addition;
/// no implicit reset).
///
/// **Serde defaults**: `cache_write_tokens` and `cost_usd` use `#[serde(default)]`
/// so that settings files written before these fields existed deserialize
/// successfully with zero values for the new fields.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProjectTokenTotals {
    /// Total prompt tokens across all accumulated turns.
    #[serde(default)]
    pub tokens_in: TokenCount,
    /// Total completion tokens across all accumulated turns.
    #[serde(default)]
    pub tokens_out: TokenCount,
    /// Total cache-read tokens across all accumulated turns.
    #[serde(default)]
    pub tokens_cached: TokenCount,
    /// Total cache-write tokens across all accumulated turns.
    #[serde(default)]
    pub cache_write_tokens: TokenCount,
    /// Total accumulated cost in USD across all turns. `0.0` when cost data
    /// was not available from the SDK.
    #[serde(default)]
    pub cost_usd: UsdCost,
}

/// Point-in-time snapshot of the context window usage for one session.
///
/// Produced from `SessionEventData::SessionUsageInfo(SessionUsageInfoData)`.
/// Only the most-recent snapshot is retained - the actor replaces
/// `last_context` on each `RecordContext` command. SDK `f64` fields are cast
/// to `u64`/`usize`; values default to zero when the SDK emits `0.0`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContextUsageStats {
    /// Tokens currently occupying the context window
    /// (`SessionUsageInfoData.current_tokens` cast to `u64`).
    pub current_tokens: TokenCount,
    /// Maximum token capacity of the context window
    /// (`SessionUsageInfoData.token_limit` cast to `u64`).
    pub token_limit: TokenCount,
    /// Number of messages currently in the context
    /// (`SessionUsageInfoData.messages_length` cast to `usize`).
    pub messages_length: Count,
}

/// Explicit type tag for a saved message record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MessageType {
    /// User-typed message.
    User,
    /// Tool-produced result; the inner value identifies which tool ran.
    Tool(ToolName),
    /// Assistant text not directly from a live LLM call.
    Assistant,
    /// Assistant text from a live LLM call; carries generation metadata.
    LlmResponse(LlmUsage),
    /// Error produced during an agent turn.
    Error,
    /// In-session event marker.
    System,
}

/// A persisted message paired with its explicit type tag.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MessageRecord {
    /// Explicit type and optional metadata for this message.
    pub message_type: MessageType,
    /// The full message value (role, content, timestamp).
    pub message: Message,
}

/// A single event emitted by the LLM streaming actor on a per-request channel.
///
/// Flows through a dedicated `mpsc::channel<StreamChunk>` from `LlmActor` to
/// `AgentActor`. Each request receives its own channel - no broadcast fan-out.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamChunk {
    /// A text token from the LLM response stream.
    Token(OutputText),
    /// A tool call the LLM wants to invoke.
    ToolCall {
        /// Provider-assigned tool call identifier (e.g. `"call_abc123"`).
        ///
        /// Must be echoed back as `tool_call_id` on the corresponding tool
        /// result message so the OpenAI-compatible wire protocol can correlate
        /// requests and results. Anthropic uses `"toolu_01..."` style IDs.
        id: ToolCallId,
        name: ToolName,
        arguments: serde_json::Value,
    },
    /// Signals that the LLM response stream is complete.
    Done,
    /// LLM generation metadata emitted after the last token and before `Done`.
    ///
    /// Each provider emits exactly one `Usage` chunk per request, carrying the
    /// token counts and model name from the response. The agent actor captures
    /// this chunk and stores it on the final assistant `MessageRecord`.
    Usage(LlmUsage),
    /// A transport or parse error from the streaming layer.
    Error(OutputText),
    /// The API returned HTTP 429 (rate limit). The inner value is the number of
    /// seconds the provider will wait before retrying. Sent to the agent so the
    /// TUI can display a notice; the actual sleep happens in the provider task.
    RateLimitRetry(WaitSecs),
}

// ── ToolCall ──────────────────────────────────────────────────────────────────

/// A tool call request extracted from a `StreamChunk::ToolCall`.
///
/// Produced by `build_tool_call`; consumed by `ToolHandle::execute` in the agent.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// Provider-assigned tool call identifier (e.g. `"call_abc123"`).
    ///
    /// Echoed back as `tool_call_id` on the tool result message so the
    /// OpenAI-compatible wire protocol can correlate requests and results.
    pub id: ToolCallId,
    /// Tool name as provided by the LLM.
    pub name: ToolName,
    /// Arguments JSON as provided by the LLM.
    pub arguments: serde_json::Value,
}

// ── Cache types ───────────────────────────────────────────────────────────────

/// A single file's path and its read-from-disk content.
///
/// Used as the leaf element of a `CachedTier`. Both fields are plain owned
/// values so `CacheSnapshot` can be cloned cheaply and sent across tasks.
#[derive(Debug, Clone)]
pub struct CachedFile {
    /// Absolute path to the source file.
    pub path: PathBuf,
    /// Full UTF-8 contents of the file at the time of the last read.
    pub content: CachedFileContent,
}

/// A group of source files assigned to the same cache tier.
///
/// Files in tier 1 are the most stable (dep-tree roots). Files in the last
/// tier are the least stable (closest to the working target). Each tier maps
/// to one Anthropic `cache_control: ephemeral` content block.
#[derive(Debug, Clone)]
pub struct CachedTier {
    /// Human-readable tier label, e.g. `"Foundation (tier 1)"`.
    pub label: StatusLabel,
    /// Source files belonging to this tier, in no guaranteed order.
    pub files: Vec<CachedFile>,
}

/// A complete snapshot of all tiered file content to be injected into the
/// Anthropic system message.
///
/// `tiers` is ordered tier 1 → tier N (most stable first). The Anthropic
/// provider iterates this slice to build the content block array.
#[derive(Debug, Clone)]
pub struct CacheSnapshot {
    /// Ordered list of tiers. Tier 1 is first (most stable). Maximum 4 tiers.
    pub tiers: Vec<CachedTier>,
}

// ── ModelOption ───────────────────────────────────────────────────────────────

/// An available Copilot model with display info and billing tier.
///
/// Populated by `client.list_models()` in `CopilotChatActor` at session
/// startup. Emitted on the `ModelsAvailable` output event and cached in
/// `state.prompt.available_models` for synchronous access during hint refresh
/// when the user types `/model`.
#[derive(Clone, Debug, bon::Builder)]
pub struct ModelOption {
    /// The SDK model identifier for `session.set_model()` calls.
    pub id: ModelId,
    /// Human-readable display name shown in the `/model` picker.
    pub display_name: ModelLabel,
    /// Maximum context length in tokens for this model.
    /// 0 means use the provider's default.
    #[builder(default)]
    pub max_context_length: TokenCount,
    /// Fraction of oldest tool-result messages to strip during compaction (0.0-1.0).
    /// 0.0 means use the provider's default.
    #[builder(default)]
    pub tool_compaction_ratio: ToolResultStripFraction,
    /// Maximum tool-call iterations before the task stops with a failure.
    /// 0 means use the provider's default.
    #[builder(default)]
    pub max_tool_iterations: Count,
    /// Target token count after compaction. Compaction trims messages to this target.
    /// 0 means use the provider's default.
    #[builder(default)]
    pub compaction_target: TokenCount,
    /// Token threshold that triggers automatic compaction toward compaction_target.
    /// When the estimated request tokens exceed this value, compaction is triggered.
    /// 0 means use the provider's default.
    #[builder(default)]
    pub auto_compact_threshold: TokenCount,
}

// ── AgentOutput ───────────────────────────────────────────────────────────────

/// Events emitted by the agent actor on its broadcast output channel.
///
/// Flows from `AgentActor` to `TuiActor` (and any other subscribers) via
/// `broadcast::channel<AgentOutput>`. Every turn ends with `Done`, `Error`,
/// or `Interrupted`. The TUI acts on each variant as part of the turn lifecycle.
///
/// The `ExecutorActor` also emits on this channel so the supervisor can observe
/// executor progress without a separate channel type.
#[derive(Clone, Debug)]
pub enum AgentOutput {
    /// A streaming text token to display immediately.
    Token(OutputText),
    /// The agent turn is complete; no more tokens for this turn.
    Done,
    /// An unrecoverable error occurred during this turn.
    Error(OutputText),
    /// Emitted between successive LLM replies within a single turn (i.e., when
    /// the LLM produced text before a tool call and then produced more text after
    /// the tool result). The TUI renders this as a blank line separator so each
    /// assistant message is visually distinct.
    MessageBreak,
    /// The turn was cancelled via the interrupt signal before completion.
    ///
    /// Emitted by `process_turn` when `consume_stream` returns an interruption
    /// error. The TUI actor uses this to show `[stopped]` in the output pane
    /// and clear `is_thinking` - but only if `is_thinking` is still true (to avoid
    /// double output when the cancel key handler already showed `[stopped]`).
    Interrupted,
    /// Emitted just before a tool call is executed. The TUI shows this as a dimmed
    /// line in the output pane and updates the thinking row label.
    ToolCallStarted {
        /// The name of the tool being called.
        name: ToolName,
        /// The arguments passed to the tool.
        args: serde_json::Value,
    },
    /// Emitted by `ExecutorActor` when the CLI session becomes idle.
    ///
    /// Signals the `SupervisorActor` that the executor has finished processing
    /// the current step's prompt and is ready for the next prompt. The supervisor
    /// uses this to advance the plan tree to the next pending leaf.
    TurnComplete,
    /// Emitted by `ExecutorActor` when the `update_plan_step` tool fires.
    ///
    /// The executor registers `update_plan_step` on the CLI session. When the
    /// CLI agent calls it, the executor sends this event so the supervisor can
    /// update the plan tree node status and notes in place.
    PlanNodeUpdate {
        /// The plan node whose status changed.
        node_id: crate::domain::plan_tree::PlanNodeId,
        /// New execution status for the node.
        status: crate::domain::plan_tree::NodeStatus,
        /// Optional notes (failure reason or completion summary).
        notes: Option<OutputText>,
    },
    /// Model update reported by the Copilot SDK or executor for a completed assistant turn.
    ///
    /// Emitted by `ExecutorActor` and `CopilotChatActor` when the SDK reports
    /// the model name via the `AssistantUsage` event. The TUI updates
    /// `status.model_display` on receipt when `model` is `Some`.
    UsageUpdate {
        /// Model identifier from the SDK usage event, when reported.
        ///
        /// Present when the Copilot SDK includes `model` in `AssistantUsageData`.
        /// `None` for non-Copilot providers (Anthropic, OpenAI) and when the SDK
        /// omits the field. When `Some`, the TUI replaces `model_display` with
        /// this value so the status bar shows the actual model name after the
        /// first turn completes.
        model: Option<ModelId>,
    },
    /// Emitted when a tool execution completes.
    ///
    /// Emitted by `AgentActor` (internal tools) and `CopilotChatActor` (SDK tools).
    /// The TUI uses `session_log` to fill in the friendly summary line above the
    /// `→ tool: args` detail line. `session_log` is `None` for SDK-side tools since
    /// `ToolResultContent` does not carry it back through the event stream.
    ToolCallCompleted {
        /// The name of the tool that completed.
        name: ToolName,
        /// Whether the tool execution succeeded.
        success: ExecutionSuccess,
        /// Optional text output from the tool, when the SDK provides it.
        result: Option<OutputText>,
        /// Human-readable summary for the TUI tool-summary line. `None` when not
        /// available (SDK path) or on error results.
        session_log: Option<OutputText>,
    },
    /// The model's stated intention before executing tool calls (AssistantIntent).
    ///
    /// Emitted by both `CopilotChatActor` and `ExecutorActor` when the SDK fires
    /// an `AssistantIntent` event. The TUI renders this as a plain output line
    /// immediately above the subsequent tool-call lines so the user can see what
    /// the model intends to do before the tool executions appear.
    IntentMessage(OutputText),
    /// A live progress update from a running tool execution (ToolExecutionProgress).
    ///
    /// Emitted when the SDK fires `ToolExecutionProgress`. Carries the SDK-assigned
    /// `tool_call_id` for future correlation with the originating `ToolCallStarted`
    /// event. The TUI renders this as a dimmed indented line under the active tool
    /// call, prefixed with `↻`.
    ToolProgress {
        /// SDK-assigned identifier for the tool call that produced this update.
        tool_call_id: ToolCallId,
        /// Human-readable progress description from the tool.
        message: OutputText,
    },
    /// A streaming partial output chunk from a running tool execution (ToolExecutionPartialResult).
    ///
    /// Emitted when the SDK fires `ToolExecutionPartialResult`. Carries the
    /// `tool_call_id` for future correlation. The TUI renders each chunk as one
    /// or more dimmed indented lines under the active tool call, split on newlines
    /// so multi-line chunks are displayed correctly.
    ToolPartialResult {
        /// SDK-assigned identifier for the tool call that produced this chunk.
        tool_call_id: ToolCallId,
        /// Partial output text, which may contain newlines.
        output: OutputText,
    },
    /// A system-level notification to display with a wall-clock timestamp.
    ///
    /// Emitted by `CopilotChatActor` for compaction lifecycle events
    /// (`SessionCompactionStart`) and by the TUI actor for slash-command feedback
    /// (e.g., `/stop`, `/switch`). The TUI renders these lines with a dimmed
    /// timestamp prefix identical to user input lines.
    SystemMessage(OutputText),
    /// Successful compaction of the session context window.
    ///
    /// Carries a human-readable summary (`text`).
    ///
    /// Emitted by `event_mapper` for `SessionCompactionComplete` on success.
    CompactionComplete {
        /// Human-readable compaction summary (e.g., "context compacted: 50000 → 12500 tokens").
        text: OutputText,
    },
    /// Available models fetched from the Copilot SDK at session startup.
    ///
    /// Emitted by `CopilotChatActor` after `client.list_models()` succeeds.
    /// The TUI stores the list in `state.prompt.available_models` for
    /// synchronous access during `/model` completion hint refresh.
    ModelsAvailable(Vec<ModelOption>),
    /// The active Copilot model has changed.
    ///
    /// Emitted by `CopilotChatActor` after session creation and after a
    /// successful `session.set_model()` call. Carries the model id used as
    /// the status-bar display label. The TUI updates
    /// `state.status.model_display` on this event.
    ActiveModelChanged(ModelId),
    /// The LLM provider entered exponential backoff after a "requests exceeded" 429.
    ///
    /// Emitted by `AgentActor` when `StreamChunk::RateLimitRetry` arrives and
    /// `is_requests_exceeded` is true. Carries the full wait duration so the TUI
    /// can compute a countdown deadline. The TUI stores the deadline in
    /// `state.status.context_window.backoff_until` and shows `| [Backoff: Xs]`
    /// in the status bar. Cleared when `Done`, `Error`, or `Interrupted` arrives.
    BackoffStarted(WaitSecs),
    /// Accumulated token totals snapshot from the token-tracker actor.
    ///
    /// Emitted by the TUI actor's periodic tick (via `token_tracker.snapshot()`) and
    /// dispatched through the broadcast channel so `apply_agent_output` can update
    /// `state.status.token_totals`.
    ///
    /// # Postcondition
    ///
    /// After applying `UsageSnapshot(totals)`, `state.status.token_totals == totals`.
    UsageSnapshot(ProjectTokenTotals),
}

// ── SupervisorEvent ───────────────────────────────────────────────────────────

/// Events emitted by the supervisor actor on its broadcast event channel.
///
/// Flows from `SupervisorActor` to `TuiActor` and any other subscribers via
/// `broadcast::channel<SupervisorEvent>`. The TUI plan panel renders the live
/// tree state by replaying these events against the initial `PlanGenerated`
/// snapshot. The executor emits `AgentOutput` events on a separate channel.
#[derive(Clone, Debug)]
pub enum SupervisorEvent {
    /// The plan tree has been generated from a goal and is ready for display.
    ///
    /// Carries an `Arc` so every subscriber gets the same allocation - no
    /// per-subscriber clone of the full tree. The TUI holds the `Arc` as its
    /// initial render snapshot, updating it via subsequent step events.
    PlanGenerated(Arc<PlanTree>),
    /// A leaf node has started executing. The TUI updates its status indicator.
    StepStarted(PlanNodeId),
    /// A leaf node completed successfully. The TUI marks the node done.
    StepCompleted(PlanNodeId),
    /// A leaf node failed. Execution halts after this event.
    StepFailed {
        /// The node that failed.
        id: PlanNodeId,
        /// Human-readable failure reason from the phase gate evaluation.
        reason: OutputText,
    },
    /// A checkpoint has been triggered (explicit marker or heuristic threshold).
    ///
    /// The config indicates which actions (commit / compact) are being taken.
    CheckpointTriggered(CheckpointConfig),
    /// All pending leaf nodes have been executed successfully.
    ExecutionComplete,
    /// The supervisor encountered an unrecoverable error or was cancelled.
    Failed {
        /// Human-readable reason for the failure.
        reason: OutputText,
    },
    /// A display-only `AgentOutput` event forwarded from the executor during
    /// step execution (e.g. `IntentMessage`, `ToolProgress`, `ToolPartialResult`).
    ///
    /// The supervisor's drain loop re-emits these events so they reach the TUI
    /// output pane while execution is in progress. The TUI handles this variant
    /// by calling `apply_agent_output` directly, preserving the same rendering
    /// path as the copilot actor.
    DisplayOutput(AgentOutput),
}

// ── FileCompletion ────────────────────────────────────────────────────────────

/// A candidate file path shown in the `@` completion hint list.
///
/// Produced by `FileScannerActor` and stored in `PromptCompletions::files`.
/// Rendered by `render_file_hints` in the TUI above the input area when the
/// buffer contains an `@` token. On selection, `path` is inserted into the
/// buffer; `display_name` is shown in the hint list for readability.
#[derive(Clone, Debug, PartialEq)]
pub struct FileCompletion {
    /// Relative or absolute filesystem path passed to the Copilot SDK as
    /// `UserMessageAttachment.path` on submit.
    pub path: FilePath,
    /// Filename portion of `path` (last path segment), shown in the hint row.
    pub display_name: FileDisplayName,
}

// ── CommandDef / CommandOutcome ───────────────────────────────────────────────

/// Metadata for a single slash command.
///
/// Used by the command registry to describe available commands and by the TUI
/// actor to generate hint lines displayed above the input area.
#[derive(Copy, Clone, Debug, PartialEq, bon::Builder)]
pub struct CommandDef {
    /// Short command name without the leading slash, e.g. `"quit"`.
    pub name: &'static str,
    /// Full usage string shown in hints, e.g. `"/quit"` or `"/switch <name>"`.
    pub usage: &'static str,
    /// One-line description shown alongside the usage in the hint area.
    pub description: &'static str,
}

/// Result returned by `CommandRegistry::execute` for a submitted prompt.
///
/// The TUI actor pattern-matches on this to decide what action to take:
/// `Quit` → exit the event loop, `SwitchEndpoint` → update the session,
/// `SystemMessage` → push formatted text to the output pane,
/// `NotACommand` → forward the text to the agent, `UnknownCommand` → show an error,
/// `CompactSession` → forward a compact request to the active chat provider,
/// `StopExecution` → interrupt the currently running agent turn,
/// `CommitChanges` → send a commit instruction message to the agent via the SDK,
/// `PushBranch` → send a push instruction message to the agent via the SDK,
/// `SelectModel` → switch to a specific model,
/// `SelectAutoModel` → revert to CLI auto-selection by calling `set_model("")`,
/// `NewSession` → save current session, reset persistence, and start a fresh SDK session,
/// `OpenAskPanel` → open the side-channel ask panel overlay.
#[derive(Clone)]
pub enum CommandOutcome {
    /// The user typed `/quit` or `/exit`; the TUI should exit.
    Quit,
    /// The user typed `/switch <name>`; the TUI should update the active endpoint.
    SwitchEndpoint(EndpointName),
    /// A command produced a displayable message (e.g. `/help` output).
    SystemMessage(OutputText),
    /// The text does not start with `/`; the TUI should submit it to the agent.
    NotACommand,
    /// The text starts with `/` but does not match any registered command.
    UnknownCommand,
    /// The user typed `/compact`; the TUI should forward a compact request to the agent.
    CompactSession,
    /// The user typed `/stop`; the TUI should interrupt the current agent turn.
    StopExecution,
    /// The user typed `/commit`; the TUI should send a commit instruction to the agent.
    CommitChanges,
    /// The user typed `/push`; the TUI should send a push instruction to the agent.
    PushBranch,
    /// The user typed `/model <id>`; the TUI should switch the active Copilot model.
    ///
    /// The TUI calls `handles.agent.set_model(&id)` which routes the command
    /// to `CopilotChatActor` via `CopilotChatCmd::SetModel`. A no-op on
    /// non-Copilot providers.
    SelectModel(ModelId),
    /// The user typed bare `/model` or `/model ` with no id; the TUI should
    /// trigger CLI auto-selection by calling `handles.agent.set_model("")`.
    ///
    /// Produced by the registry when no model id follows the command, allowing
    /// the Copilot headless CLI to choose the model automatically.
    SelectAutoModel,
    /// The user typed `/run-plan <path>`; the TUI should load and start the
    /// named guided plan file. The inner `String` is the raw path argument.
    RunPlan(FilePath),
    /// The user typed `/new-session`; the TUI should save the current session,
    /// reset the persistence handle to a new UUID, ask the Copilot actor to
    /// create a fresh SDK session, and clear the output pane.
    NewSession,
    /// The user typed `/ask`; the TUI should open the side-channel ask panel overlay.
    ///
    /// The TUI sets `interaction.ask_panel = Some(AskPanelState::default())` and
    /// switches `interaction.input_focus = InputFocus::Ask`. The ask actor is seeded
    /// with a snapshot of the current main conversation history.
    OpenAskPanel,
    /// Launch a scoped background SDK agent session with the given name and prompt.
    /// Streams `AgentFeedOutput` events to the `AgentFeed` panel.
    RunBackgroundAgent {
        agent: AgentName,
        prompt: PromptText,
    },
    /// The user typed `/run-pipeline`; start the deterministic orchestrator pipeline.
    /// The feature context (message + attachments) is extracted from the submission
    /// text in `start_pipeline`.
    StartPipeline {
        /// When `true`, skip already-completed steps (--resume flag was present).
        resume: bool,
    },
    /// The user typed `/generate-catalog [--provider <name>]`; fetch and display model catalog.
    GenerateCatalog {
        /// Optional provider name filter (e.g., Some("openrouter")).
        provider: Option<String>,
    },
}

// ── AgentFeedOutput ───────────────────────────────────────────────────────────

/// Events produced by background tasks for display in the agent feed panel.
///
/// Pushed through `agent_feed_tx` by any background task that wants to
/// surface live status in the TUI agent feed panel.
#[derive(Clone, Debug)]
pub enum AgentFeedOutput {
    /// A task has started. The label appears in the panel title thinking row.
    TaskStarted {
        name: AgentName,
        /// Optional display label for the model running this agent step.
        model: Option<ModelLabel>,
    },
    /// A plain-text status line to append to the feed.
    StatusLine(OutputText),
    /// A tool event line (start/progress/complete) to append as a separate line.
    ToolEventLine(OutputText),
    /// Marks the end of a streamed assistant message. Flushes `pending_status_message`
    /// and `pending_tool_event` so the committed line appears before the next tool event.
    MessageBreak,
    /// A task has completed successfully.
    TaskCompleted { name: AgentName },
    /// A task has failed with an error message.
    TaskFailed { name: AgentName, reason: OutputText },
    /// Clear all content from the agent feed panel.
    Clear,
}

// ── FeedId / FeedEntry / RouteResult ──────────────────────────────────────────

/// Identifies which feed an SDK event belongs to.
///
/// `Agent(String)` carries the outer `"task"` tool_call_id that spawned the
/// background agent. `MainConversation` is for main-session events.
/// `AskPanel` is reserved for the future ask-panel feature.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FeedId {
    /// The primary conversation feed for the active session.
    MainConversation,
    /// A background agent feed, keyed by the tool_call_id that spawned the agent.
    Agent(ToolCallId),
    /// Reserved for the future ask-panel feature.
    AskPanel,
}

/// Bundles a `FeedId` with an `AgentFeedOutput` for delivery to the correct feed channel.
#[derive(Debug, Clone)]
pub struct FeedEntry {
    /// The target feed for this output event.
    pub feed_id: FeedId,
    /// The output event to deliver to the feed.
    pub output: AgentFeedOutput,
}

impl From<AgentFeedOutput> for FeedEntry {
    fn from(output: AgentFeedOutput) -> Self {
        FeedEntry {
            feed_id: FeedId::Agent(ToolCallId::from("legacy-agent-feed")),
            output,
        }
    }
}

/// Return type of `FeedRouter::route_event`: the main-feed output and optional feed entry.
#[derive(Debug)]
pub struct RouteResult {
    /// Output destined for the main conversation feed, if any.
    pub main_out: Option<AgentOutput>,
    /// Output destined for a specific agent feed, if any.
    pub feed_out: Option<FeedEntry>,
}

/// A message automatically generated by the orchestrator to be fed back to the LLM
/// as if the user had typed it. Wraps an [`OutputText`].
#[derive(Debug, Clone)]
pub struct AutomatedUserMessage(pub OutputText);
