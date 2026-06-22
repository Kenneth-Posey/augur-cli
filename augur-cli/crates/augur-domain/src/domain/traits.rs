//! Cross-cutting trait abstractions shared by multiple actor layers.
//!
//! Traits live here so actor modules can depend on the abstraction without
//! importing from sibling actor crates. Only `wiring.rs` injects concrete
//! types that implement these traits.

use crate::domain::lsp::LspError;
use crate::domain::task_types::AgentSpecName;
use crate::domain::{
    AgentName, AgentOutput, CacheSnapshot, EndpointName, FilePath, Message, MessageRecord, ModelId,
    PromptText, SdkSessionId, StreamChunk, ToolCall, ToolCallResult, ToolDefinition,
};
use tokio::sync::{broadcast, mpsc};

/// Bundles all inputs for a single streaming completion request.
#[derive(Clone, Debug, bon::Builder)]
pub struct CompletionRequest {
    /// Target endpoint to route the request to.
    pub endpoint: EndpointName,
    /// Full message history for the completion.
    pub messages: Vec<Message>,
    /// Tool schemas exposed to the model for this completion.
    pub tools: Vec<ToolDefinition>,
    /// Optional cache snapshot for Anthropic tiered system-message injection.
    pub cache: Option<CacheSnapshot>,
    /// Optional model override. When set, overrides the endpoint's configured model for this request.
    pub model_override: Option<ModelId>,
}

/// Abstraction over a streaming LLM completion source.
///
/// Implemented by `LlmHandle` (real actor) and fake types in tests. Allows
/// `AgentActor` to be generic over the LLM backend so tests do not need to
/// spawn a real `LlmActor`. Each call creates a fresh per-request channel.
///
/// Defined in `domain/traits.rs` so the agent actor depends on this abstraction
/// without importing from `actors::llm`. `actors::llm::handle` re-exports it.
pub trait LlmClient: Send + Sync + 'static {
    /// Submit a completion request and return the per-request stream receiver.
    ///
    /// Returns a channel receiver that will yield `StreamChunk` events until
    /// `StreamChunk::Done` or `StreamChunk::Error`. The receiver is owned by
    /// the caller; no shared state exists between concurrent requests.
    /// `cache` is forwarded to the Anthropic provider for tiered system message
    /// injection; other providers ignore it.
    fn complete_stream(&self, request: CompletionRequest) -> mpsc::Receiver<StreamChunk>;
}

/// Abstraction over a tool execution backend.
///
/// Implemented by `ToolHandle` (real actor) and fake types in tests. Allows
/// `AgentActor` to be generic over tool dispatch so tests do not need a real
/// `ToolActor`. The `definitions` method returns the immutable tool schema
/// snapshot for inclusion in LLM requests.
///
/// Defined in `domain/traits.rs` so the agent actor depends on this abstraction
/// without importing from `actors::tool`. `actors::tool::handle` re-exports it.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync + 'static {
    /// Return all registered tool schemas for inclusion in LLM requests.
    fn definitions(&self) -> &[ToolDefinition];
    /// Execute a tool call; returns the result or a transport error.
    async fn execute(&self, call: ToolCall) -> anyhow::Result<ToolCallResult>;
}

/// Abstraction over an LSP request/response backend.
///
/// Implemented by `LspHandle` in `actors::lsp`. Defined in `domain/` so
/// tool implementations can depend on this contract without importing from
/// `actors`.
#[async_trait::async_trait]
pub trait LspClient: Send + Sync + 'static {
    /// Submit one JSON-RPC request and await exactly one response.
    async fn request(
        &self,
        method: String,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, LspError>;
}

/// Operational mode for the executor backend session.
///
/// Sent via `ExecutorDriver::set_mode` to control how the CLI session
/// interprets subsequent prompts. `Interactive` is the default mode for
/// one-off queries. `Plan` enables step-driven plan execution. `Autopilot`
/// allows the session to run without awaiting user confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutorMode {
    /// Standard one-off interactive query mode.
    Interactive,
    /// Step-driven plan execution mode; the session executes plan node prompts.
    Plan,
    /// Fully autonomous mode; no user confirmation required between steps.
    Autopilot,
}

/// Abstraction over a background task launcher for non-Copilot endpoints.
///
/// Implemented by `OpenRouterTaskRunner` in wiring. Injected into
/// `EndpointRoutingChatProvider` to keep the chat provider testable.
pub trait BackgroundTaskRunnerPort: Send + Sync + 'static {
    /// Fire-and-forget spawn of a background agent task.
    ///
    /// Inputs: `agent` - spec name of the agent to load and run;
    /// `prompt` - the initial user prompt submitted to the task.
    /// Side effects: spawns a Tokio task; does not await completion.
    fn run(&self, agent: AgentSpecName, prompt: PromptText);
}

/// Abstraction over a chat backend: either `AgentHandle` or `CopilotChatHandle`.
///
/// Implemented by `AgentHandle` (standard LLM path) and `CopilotChatHandle`
/// (GitHub Copilot SDK path). The TUI actor holds `Arc<dyn ChatProvider>` so
/// it is not coupled to either concrete type. `wiring.rs` selects which
/// implementation to inject based on `config.copilot_chat.enabled`.
///
/// All methods are sync. `submit` and `shutdown` use `try_send` internally
/// so callers never block. `subscribe_output` returns a new broadcast receiver
/// by value with no blocking.
pub trait ChatProvider: Send + Sync + 'static {
    /// Submit a user prompt for a new conversation turn.
    ///
    /// `endpoint` is forwarded to `AgentHandle` for routing; `CopilotChatHandle`
    /// ignores it because the Copilot session owns its own model selection.
    /// Non-blocking: uses `try_send` and silently drops on a full channel.
    fn submit(&self, prompt: PromptText, endpoint: Option<EndpointName>);

    /// Signal the currently running turn to stop.
    ///
    /// For `AgentHandle`, sends `true` on the cancel watch channel.
    /// For `CopilotChatHandle`, no-op - Copilot sessions do not support mid-turn
    /// cancellation via the SDK at this time.
    fn interrupt(&self);

    /// Send a graceful shutdown signal to the underlying actor.
    ///
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    fn shutdown(&self);

    /// Restore a previously saved session by replaying conversation history.
    ///
    /// For `AgentHandle`, sends `AgentCommand::RestoreSession` so the agent
    /// rebuilds its `ConversationHistory` from the supplied records.
    /// For `CopilotChatHandle`, this is a no-op - the Copilot SDK owns session
    /// context and does not support external history injection.
    fn restore(&self, records: Vec<MessageRecord>);

    /// Subscribe to the output broadcast channel.
    ///
    /// Returns a new `broadcast::Receiver<AgentOutput>`. The TUI actor calls
    /// this at spawn time. Each subscriber receives all events emitted after
    /// the subscription is created.
    fn subscribe_output(&self) -> broadcast::Receiver<AgentOutput>;

    /// Request the active session to compact its conversation context window.
    ///
    /// For `CopilotChatHandle`, sends `CopilotChatCmd::Compact` to the actor
    /// which forwards `/compact` to the GitHub Copilot SDK session.
    /// For `AgentHandle` and other providers that do not support compaction,
    /// this is a no-op by default.
    fn compact(&self) {}

    /// Submit a user prompt with file attachments for a new conversation turn.
    ///
    /// `attachments` is a list of `FilePath` values parsed from `@token` syntax
    /// in the user's buffer. `CopilotChatHandle` overrides this method to pass
    /// attachments through the Copilot SDK `MessageOptions::attachments` field.
    ///
    /// The default implementation ignores `attachments` and falls back to a
    /// plain `submit(prompt, endpoint)` call, preserving backward compatibility
    /// for `AgentHandle` and test doubles that do not override the method.
    fn submit_with_attachments(
        &self,
        prompt: PromptText,
        endpoint: Option<EndpointName>,
        _attachments: Vec<FilePath>,
    ) {
        self.submit(prompt, endpoint);
    }

    /// Switch the active model for the underlying session.
    ///
    /// For `CopilotChatHandle`, sends `CopilotChatCmd::SetModel` to the actor
    /// which calls `session.set_model()` on the SDK session. For providers that
    /// do not support runtime model switching this is a no-op by default.
    fn set_model(&self, _model_id: ModelId) {}

    /// Switch the active model with an explicit reasoning effort level.
    ///
    /// For `CopilotChatHandle`, sends `CopilotChatCmd::SetModel` with a
    /// `reasoning_effort` field so the actor can pass it through to the SDK's
    /// `SetModelOptions`. The default implementation falls back to `set_model`
    /// for providers that do not override thinking mode.
    fn set_model_with_options(
        &self,
        model_id: ModelId,
        _reasoning_effort: Option<crate::domain::thinking_mode::ReasoningEffort>,
    ) {
        self.set_model(model_id);
    }

    /// Replace the active SDK session with a new or resumed one.
    ///
    /// For `CopilotChatHandle`, sends `CopilotChatCmd::ReplaceSession` to the
    /// actor, which closes the current SDK session and either resumes the
    /// specified session (`Some(id)`) or creates a fresh one (`None`).
    /// Called by `apply_restored_session` when the picker loads a session with a
    /// linked SDK session ID, and by the `/new-session` command handler.
    /// For `AgentHandle` and other providers that do not use an SDK session
    /// this is a no-op by default.
    fn replace_session(&self, _sdk_session_id: Option<SdkSessionId>) {}

    /// Launch a background SDK agent session and stream output to the feed panel.
    ///
    /// For `CopilotChatHandle`, sends `CopilotChatCmd::RunBackgroundAgent` to the
    /// actor which spawns a scoped SDK session. For `AgentHandle` and other
    /// providers that do not support background sessions this is a no-op by default.
    fn run_background_agent(&self, _agent: AgentName, _prompt: PromptText) {}
}

/// Abstraction over an executor backend (CLI session driver).
///
/// Implemented by `ExecutorHandle` (concrete actor) and test doubles.
/// Defined in `domain/` so the supervisor actor can depend on it without
/// importing from `actors::executor`. Only `wiring.rs` passes the concrete
/// `ExecutorHandle` to the supervisor.
///
/// All methods are `async` because the underlying channel send may yield.
/// `subscribe_output` is sync because `broadcast::Receiver` is returned by value.
#[async_trait::async_trait]
pub trait ExecutorDriver: Send + Sync + 'static {
    /// Send a plain-text prompt to the CLI session for execution.
    ///
    /// The session processes the prompt and emits `AgentOutput` events on
    /// the broadcast channel. The supervisor calls this once per plan step.
    async fn send_prompt(&self, content: PromptText);

    /// Switch the CLI session into the given operational mode.
    ///
    /// Should be called before the first `send_prompt` of a plan run to set
    /// `Plan` mode. Call with `Interactive` to restore normal behavior.
    async fn set_mode(&self, mode: ExecutorMode);

    /// Ask the CLI session to compact its conversation context.
    ///
    /// Called by the supervisor at checkpoint nodes that carry `compact: true`.
    /// The session emits a `TurnComplete` event when compaction finishes.
    async fn compact(&self);

    /// Subscribe to the executor output stream.
    ///
    /// Each subscriber receives all `AgentOutput` events emitted after the
    /// call. The supervisor is the primary subscriber; the TUI may also
    /// subscribe to forward executor tokens for display.
    fn subscribe_output(&self) -> broadcast::Receiver<AgentOutput>;
}
