//! Agent actor: orchestrates LLM calls, tool execution, and conversation history.

use super::agent_actor_ops as actor_ops;
use super::agent_ops::{AgentOutput, TurnConfig};
use super::assistant_core::{self, TurnContext, TurnResult};
use super::handle::AgentHandle;
use super::history::ConversationHistory;
use crate::actors::history_adapter::handle::HistoryAdapterHandle;
use crate::actors::token_tracker::TokenTrackerHandle;
use crate::persistence::handle::PersistenceHandle;
use augur_domain::config::types::AgentConfig;
use augur_domain::domain::channels::AGENT_COMMAND_CAPACITY;
use augur_domain::domain::task_types::AgentExtensions;
use augur_domain::domain::{CancelSignal, LlmClient, Message, ToolExecutor};
use augur_domain::domain::{EndpointName, OutputText};
use augur_domain::persistence::types::MessageRecord;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};

/// Commands that flow into the agent actor's mpsc channel.
pub(crate) enum AgentCommand {
    /// Submit a user prompt for a new conversation turn.
    Submit {
        /// The user's input text.
        prompt: augur_domain::domain::PromptText,
        /// The endpoint to use for this turn.
        endpoint: EndpointName,
    },
    /// Replace conversation history with a previously saved session's messages.
    RestoreSession(Vec<Message>),
    /// Request a snapshot of the current conversation history.
    SnapshotHistory {
        /// Reply channel that receives the cloned message history.
        reply_tx: tokio::sync::oneshot::Sender<Vec<Message>>,
    },
    /// Clear conversation history and error annotations, starting a fresh session.
    ///
    /// Resets the agent's in-memory conversation history to a clean state while
    /// keeping the system prompt. Used by the `/new-session` command for the
    /// OpenRouter (AgentHandle) path to prevent old messages from being sent
    /// to the LLM in subsequent turns.
    ClearHistory,
    /// Compact the conversation history using the configured message compactor.
    ///
    /// When a `message_compactor` is set in AgentExtensions, this command applies
    /// it to the current history and replaces the history with the compacted result.
    /// Emits a `SystemMessage` output with a confirmation notice.
    /// No-op when no compactor is configured (e.g. non-OpenRouter endpoints).
    Compact,
    /// Set the model to use for subsequent requests.
    SetModel(augur_domain::domain::string_newtypes::ModelId),
    /// Query the current agent state (last endpoint and selected model).
    GetState {
        /// Reply channel that receives the current endpoint and model.
        reply_tx: tokio::sync::oneshot::Sender<AgentState>,
    },
    /// Gracefully stop the agent task loop.
    Shutdown,
}

/// Current agent state: the last endpoint used and the selected model override.
#[derive(Clone, Debug)]
pub struct AgentState {
    /// The last endpoint used for a submission.
    pub last_endpoint: Option<EndpointName>,
    /// The currently selected model override.
    pub selected_model: Option<augur_domain::domain::string_newtypes::ModelId>,
}

/// Channels owned by the agent run task for the lifetime of the actor.
#[derive(bon::Builder)]
pub(super) struct RunPipes {
    pub(super) cmd_rx: mpsc::Receiver<AgentCommand>,
    pub(super) output_tx: broadcast::Sender<AgentOutput>,
    pub(super) cancel_tx: Arc<watch::Sender<CancelSignal>>,
    pub(super) cancel_rx: watch::Receiver<CancelSignal>,
}

/// Supporting service handles bundled to keep `AgentSpawnArgs` within 5 fields.
#[derive(bon::Builder)]
pub struct AgentServices {
    /// Persistence handle for auto-saving session turn records.
    pub persistence: PersistenceHandle,
    /// Logger handle for appending turn messages to the session JSONL log.
    pub logger: crate::actors::logger::LoggerHandle,
    /// Token-tracker handle for recording LLM usage after each turn.
    pub token_tracker: TokenTrackerHandle,
    /// History-adapter handle for routing conversation messages to the history feed.
    pub history_adapter: HistoryAdapterHandle,
    /// Optional pre-created output broadcast sender.
    ///
    /// When `Some`, the agent shares this channel with callers (e.g. the LLM
    /// actor startup emission). When `None` the agent creates its own channel.
    pub output_tx: Option<broadcast::Sender<AgentOutput>>,
}

/// Mutable per-session state held across all commands in the run loop.
///
/// Bundles `history` and `error_annotations` so they can be passed together
/// without exceeding the 3-parameter limit on helper functions.
pub(super) struct AgentRunState {
    pub(super) history: ConversationHistory,
    pub(super) error_annotations: Vec<(augur_domain::domain::newtypes::Count, MessageRecord)>,
    /// The currently selected model override. When `None`, uses the endpoint's configured model.
    pub(super) selected_model: Option<augur_domain::domain::string_newtypes::ModelId>,
    /// The last endpoint used for a submission.
    pub(super) last_endpoint: Option<EndpointName>,
}

/// Prompt and endpoint bundled from a `Submit` command for a single turn.
pub(super) struct SubmitPayload {
    pub(super) prompt: augur_domain::domain::PromptText,
    pub(super) endpoint: EndpointName,
}

/// Arguments for spawning the agent actor.
#[derive(bon::Builder)]
pub struct AgentRuntime {
    /// Optional runtime extensions: cache handle and instruction prefix.
    pub extensions: AgentExtensions,
    /// Application configuration for resolving endpoint definitions.
    pub app_config: augur_domain::config::types::AppConfig,
    /// Maximum context length in tokens for the selected model.
    ///
    /// Used to compute the total request-size cap at `max_context_length * 0.8`.
    /// Falls back to `DEFAULT_MAX_CONTEXT_LENGTH` from `agent_ops` when zero.
    #[builder(default)]
    pub max_context_length: augur_domain::domain::newtypes::TokenCount,
    /// Token threshold that triggers the request-size guard warning.
    ///
    /// When set to a value > 0, the guard warns the LLM when estimated request
    /// tokens exceed this threshold and continues the loop (does not halt).
    /// When zero, falls back to `request_cap_for_context(max_context_length)`.
    /// Typically sourced from the model's `auto_compact_threshold` in the
    /// provider catalog (e.g. 300K for deepseek/deepseek-v4-flash).
    #[builder(default)]
    pub request_cap_threshold: augur_domain::domain::newtypes::TokenCount,
}

/// Arguments for spawning the agent actor.
#[derive(bon::Builder)]
pub struct AgentSpawnArgs<L, T> {
    /// LLM client handle for sending completion requests.
    pub llm: L,
    /// Tool executor handle for running tool calls.
    pub tools: T,
    /// Agent behaviour configuration: system prompt, max tokens, temperature.
    pub config: AgentConfig,
    /// Supporting service handles (persistence, project settings, logger).
    pub services: AgentServices,
    /// Runtime configuration that bundles extensions and app config.
    pub runtime: AgentRuntime,
}

#[derive(bon::Builder)]
pub(super) struct RestoreHistoryArgs<'a> {
    pub(super) history: &'a mut ConversationHistory,
    pub(super) error_annotations:
        &'a mut Vec<(augur_domain::domain::newtypes::Count, MessageRecord)>,
    pub(super) extended_prompt: &'a OutputText,
    pub(super) records: Vec<Message>,
    pub(super) openrouter_context_records: Option<Vec<Message>>,
}

#[derive(bon::Builder)]
pub(super) struct SubmitTurnArgs<'a, L, T> {
    pub(super) pipes: &'a mut RunPipes,
    pub(super) actor_args: &'a AgentSpawnArgs<L, T>,
    pub(super) history: &'a mut ConversationHistory,
    pub(super) request: SubmitTurnRequest,
}

#[derive(bon::Builder)]
pub(super) struct SubmitTurnRequest {
    pub(super) prompt: augur_domain::domain::PromptText,
    pub(super) endpoint: EndpointName,
    /// Optional model override from the agent's selected model.
    pub(super) model_override: Option<augur_domain::domain::string_newtypes::ModelId>,
}

#[derive(bon::Builder)]
pub(super) struct FinalizeTurnState<'a> {
    pub(super) history: &'a mut ConversationHistory,
    pub(super) error_annotations:
        &'a mut Vec<(augur_domain::domain::newtypes::Count, MessageRecord)>,
    pub(super) len_before: usize,
    pub(super) turn_result: TurnResult,
}

#[derive(bon::Builder)]
pub(super) struct FinalizeTurnArgs<'a, L, T> {
    pub(super) actor_args: &'a AgentSpawnArgs<L, T>,
    pub(super) endpoint: EndpointName,
    pub(super) state: FinalizeTurnState<'a>,
}

pub(super) struct SubmitCmdInput<'a> {
    pub(super) run_state: &'a mut AgentRunState,
    pub(super) payload: SubmitPayload,
}

/// Spawn the agent actor task and return a join handle plus a cloneable `AgentHandle`.
#[tracing::instrument(skip_all, level = "info")]
pub fn spawn<L, T>(args: AgentSpawnArgs<L, T>) -> (tokio::task::JoinHandle<()>, AgentHandle)
where
    L: LlmClient,
    T: ToolExecutor,
{
    let (cmd_tx, cmd_rx) = mpsc::channel(*AGENT_COMMAND_CAPACITY);
    let output_tx = actor_ops::resolve_output_tx(args.services.output_tx.clone());
    let (cancel_tx_raw, cancel_rx) = watch::channel(CancelSignal::Clear);
    let cancel_tx = Arc::new(cancel_tx_raw);
    let handle = AgentHandle::new(cmd_tx, output_tx.clone(), Arc::clone(&cancel_tx));
    let pipes = RunPipes::builder()
        .cmd_rx(cmd_rx)
        .output_tx(output_tx)
        .cancel_tx(cancel_tx)
        .cancel_rx(cancel_rx)
        .build();
    let join = tokio::spawn(run(pipes, args));
    (join, handle)
}

async fn run<L: LlmClient, T: ToolExecutor>(mut pipes: RunPipes, args: AgentSpawnArgs<L, T>) {
    actor_ops::run_loop(&mut pipes, &args).await;
}

/// Agentic re-entry loop: calls the LLM, executes tool calls, and loops until done.
pub(super) async fn process_turn<L: LlmClient, T: ToolExecutor>(
    history: &mut ConversationHistory,
    ctx: TurnContext<'_, L, T>,
    cfg: TurnConfig,
) -> TurnResult {
    assistant_core::process_turn(history, ctx, cfg).await
}
