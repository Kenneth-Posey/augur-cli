//! `OpenRouterTaskActor`: per-task actor that loads an agent spec, runs a
//! tool-calling loop, and emits `AgentFeedOutput` events to the TUI panel.

use super::handle::OpenRouterTaskHandle;
use super::openrouter_task_actor_ops as actor_ops;
use super::spec_loader::load_agent_spec;
use crate::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle;
use crate::compaction::{compact_messages_for_openrouter, estimate_request_tokens_for_compaction};
use crate::model_config::{ResolvedModelConfig, resolve_model_config};
use actor_ops::{
    build_task_system_prompt, is_at_iteration_limit, prepend_prefix, signal_to_feed_event,
};
use augur_domain::actors::agent::history::ConversationHistory;
use augur_domain::actors::token_tracker::TokenTrackerHandle;
use augur_domain::newtypes::Count;
use augur_domain::string_newtypes::{AgentName, ModelLabel, OutputText, StringNewtype, ToolName};
use augur_domain::task_types::{
    AgentSpecName, InstructionPrefix, RepoRoot, SpawnAgentAck, SpawnAgentHandle, SpawnAgentRequest,
    TaskDepth, TaskRunId, TaskSignal,
};
use augur_domain::tool_call_formatting::format_tool_call_line;
use augur_domain::tools::builtin::spawn_agent::SpawnAgentTool;
use augur_domain::tools::definition::ToolDefinition;
use augur_domain::traits::{CompletionRequest, LlmClient, ToolExecutor};
use augur_domain::types::{AgentFeedOutput, FeedEntry, FeedId, Message, ToolCall};
use augur_domain::{AccumulatedText, EndpointName, ModelId, NumericNewtype, PromptText};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

/// Configuration specific to one task execution instance.
///
/// Carries the agent identity, the user prompt, the current nesting depth, and
/// the one-shot channel on which the task reports its lifecycle outcome.
#[derive(bon::Builder)]
pub struct TaskConfig {
    /// User request envelope for this task run.
    pub request: TaskRequestSpec,
    /// Runtime options selected by the caller.
    pub runtime: TaskRuntimeOptions,
    /// Signal/output correlation channels for this run.
    pub correlation: TaskCorrelation,
}

/// User-facing request envelope for one OpenRouter task.
#[derive(bon::Builder)]
pub struct TaskRequestSpec {
    /// Name of the agent spec to load (maps to `<spec_base_path>/<name>.agent.md`).
    pub agent_name: AgentSpecName,
    /// The prompt to send to the agent as the initial user message.
    pub prompt: PromptText,
    /// Current nesting depth; prevents unbounded recursion.
    pub depth: TaskDepth,
}

/// Runtime options that influence request execution behavior.
#[derive(bon::Builder)]
pub struct TaskRuntimeOptions {
    /// Optional model override from the parent caller.
    pub model_override: Option<ModelId>,
}

/// Correlation channels and identifiers for one task run.
#[derive(bon::Builder)]
pub struct TaskCorrelation {
    /// Channel on which the task reports completion or failure.
    pub signal_tx: oneshot::Sender<TaskSignal>,
    /// Optional orchestrator correlation id for this run.
    pub run_id: Option<TaskRunId>,
}

/// Supporting services injected into the task actor at spawn time.
///
/// Bundles the four cross-cutting service handles so `OpenRouterTaskArgs` stays
/// within the five-field limit.
#[derive(bon::Builder, Clone)]
pub struct TaskServices {
    /// Agent feed channel for emitting status events to the TUI panel.
    pub feed_tx: mpsc::Sender<FeedEntry>,
    /// Cached instruction prefix prepended on every completion request.
    pub instruction_prefix: Arc<InstructionPrefix>,
    /// Base path for resolving agent spec files (e.g. `RepoRoot/.github/agents/`).
    pub spec_base_path: RepoRoot,
    /// Optional token tracker for recording LLM usage after each turn.
    pub token_tracker: Option<TokenTrackerHandle>,
    /// Optional OpenRouter orchestrator handle for correlated run lifecycle reporting.
    pub orchestrator: Option<OpenRouterOrchestratorHandle>,
}

/// Arguments for spawning the `OpenRouterTaskActor`.
///
/// Generic over the LLM client `L` and tool executor `T` so tests can inject
/// fake doubles without spawning real actors.
#[derive(bon::Builder)]
pub struct OpenRouterTaskArgs<L, T> {
    /// LLM client for streaming completion requests.
    pub llm: L,
    /// Tool executor containing the task's scoped tools.
    pub tools: T,
    /// Task configuration: agent name, prompt, depth, signal channel.
    pub task_config: TaskConfig,
    /// Supporting handles: feed channel, instruction prefix, spec base path.
    pub task_services: TaskServices,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Dependencies threaded through all iterations of the task loop.
struct TaskLoopDeps<'a, L, T> {
    runtime: TaskLoopRuntime<'a, L, T>,
    orchestrator: Option<OrchestratorCorrelation>,
}

struct TaskLoopRuntime<'a, L, T> {
    llm: &'a L,
    tools: &'a T,
    feed: TaskFeedTarget<'a>,
    instruction_prefix: &'a InstructionPrefix,
    model_override: Option<ModelId>,
    /// Resolved per-model configuration (budget, strip fraction, max iterations).
    model_config: ResolvedModelConfig,
}

#[derive(Clone)]
struct OrchestratorCorrelation {
    orchestrator: OpenRouterOrchestratorHandle,
    run_id: TaskRunId,
}

/// Mutable state owned by the task loop across iterations.
struct TaskLoopState<'a> {
    history: &'a mut ConversationHistory,
    tool_defs: &'a [ToolDefinition],
}

#[derive(bon::Builder)]
struct TaskLoopProgress<'a> {
    iterations: &'a mut Count,
    max: Count,
    accumulated: &'a mut String,
}

struct CompletionWithoutTool<'a> {
    history: &'a mut ConversationHistory,
    orchestrator: &'a Option<OrchestratorCorrelation>,
    text: OutputText,
    accumulated: String,
}

struct ToolIteration<'a, L, T> {
    runtime: &'a TaskLoopRuntime<'a, L, T>,
    history: &'a mut ConversationHistory,
    call: ToolCall,
    text: OutputText,
}

struct TaskFeedTarget<'a> {
    tx: &'a mpsc::Sender<FeedEntry>,
    id: &'a FeedId,
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Spawn the task actor and return a join handle plus the task handle.
///
/// The actor runs to completion (or failure) and then exits. It does not accept
/// commands after spawn - its entire configuration is supplied upfront via `args`.
/// Emits `AgentFeedOutput::TaskStarted` immediately after loading the spec, runs
/// the tool-calling loop, emits `TaskCompleted` or `TaskFailed`, and sends a
/// `TaskSignal` on the one-shot channel from `TaskConfig::signal_tx`.
///
/// # Parameters
///
/// - `args`: complete actor configuration including LLM, tools, and service handles.
///
/// # Returns
///
/// `(JoinHandle<()>, OpenRouterTaskHandle)` - the join handle for the actor task
/// and a cloneable handle wrapping the spawn-request sender.
pub fn spawn<L, T>(
    args: OpenRouterTaskArgs<L, T>,
) -> (tokio::task::JoinHandle<()>, OpenRouterTaskHandle)
where
    L: LlmClient,
    T: ToolExecutor,
{
    let (spawn_tx, spawn_rx) = mpsc::channel::<SpawnAgentRequest>(8);
    let handle = OpenRouterTaskHandle::new(spawn_tx.clone());

    // Background task to handle spawn requests from SpawnAgentTool.
    // Fails all requests with a clear message so the parent task is never
    // deadlocked when SpawnAgentTool awaits a reply.
    tokio::spawn(async move {
        let mut rx = spawn_rx;
        while let Some(req) = rx.recv().await {
            let _ = req.channels.ack_tx.send(SpawnAgentAck::Failed {
                reason: OutputText::new(
                    "sub-agent spawning requires a wired runtime; not available in this context",
                ),
            });
        }
    });

    let join = tokio::spawn(run(args, spawn_tx));
    (join, handle)
}

async fn run<L: LlmClient, T: ToolExecutor>(
    args: OpenRouterTaskArgs<L, T>,
    spawn_tx: mpsc::Sender<SpawnAgentRequest>,
) {
    let OpenRouterTaskArgs {
        llm,
        tools,
        task_config,
        task_services,
    } = args;
    let feed_id = task_feed_id(&task_config);
    let orchestrator_correlation = build_orchestrator_correlation(&task_config, &task_services);
    let agent_spec_name = task_config.request.agent_name.clone();
    let signal = match load_spec_for_task(&task_config, &task_services).await {
        Ok(spec) => {
            let tool_defs = build_tool_defs_with_spawn(&tools, &task_config, spawn_tx);
            let mut history = build_task_history(&task_config, &tool_defs, &spec.instructions);
            emit_task_started(
                TaskFeedTarget {
                    tx: &task_services.feed_tx,
                    id: &feed_id,
                },
                &agent_spec_name,
                &task_config,
            )
            .await;
            // Resolve per-model config once at task startup.
            let model_config = resolve_model_config(task_config.runtime.model_override.as_ref());
            run_task_loop(
                TaskLoopDeps {
                    runtime: TaskLoopRuntime {
                        llm: &llm,
                        tools: &tools,
                        feed: TaskFeedTarget {
                            tx: &task_services.feed_tx,
                            id: &feed_id,
                        },
                        instruction_prefix: &task_services.instruction_prefix,
                        model_override: task_config.runtime.model_override.clone(),
                        model_config,
                    },
                    orchestrator: orchestrator_correlation,
                },
                TaskLoopState {
                    history: &mut history,
                    tool_defs: &tool_defs,
                },
            )
            .await
        }
        Err(reason) => {
            emit_task_failed(
                TaskFeedTarget {
                    tx: &task_services.feed_tx,
                    id: &feed_id,
                },
                &agent_spec_name,
                reason.clone(),
            )
            .await;
            let signal = TaskSignal::Failed { reason };
            report_orchestrator_terminal(&orchestrator_correlation, &signal);
            signal
        }
    };
    let feed_event = signal_to_feed_event(&agent_spec_name, &signal);
    emit_feed(&task_services.feed_tx, &feed_id, feed_event).await;
    let _ = task_config.correlation.signal_tx.send(signal);
}

async fn run_task_loop<L: LlmClient, T: ToolExecutor>(
    deps: TaskLoopDeps<'_, L, T>,
    mut state: TaskLoopState<'_>,
) -> TaskSignal {
    report_orchestrator_launch(&deps.orchestrator);
    let max = deps.runtime.model_config.max_iterations;
    let mut iterations = Count::ZERO;
    let mut accumulated = String::new();

    loop {
        let next = run_task_loop_iteration(
            &deps,
            &mut state,
            TaskLoopProgress::builder()
                .iterations(&mut iterations)
                .max(max)
                .accumulated(&mut accumulated)
                .build(),
        )
        .await;
        if let Some(signal) = next {
            return signal;
        }
    }
}

async fn run_task_loop_iteration<L: LlmClient, T: ToolExecutor>(
    deps: &TaskLoopDeps<'_, L, T>,
    state: &mut TaskLoopState<'_>,
    progress: TaskLoopProgress<'_>,
) -> Option<TaskSignal> {
    let TaskLoopProgress {
        iterations,
        max,
        accumulated,
    } = progress;
    if is_at_iteration_limit(*iterations, max).0 {
        let reason = OutputText::new(format!("max tool iterations ({max}) reached"));
        return Some(report_failed_signal(&deps.orchestrator, reason));
    }
    *iterations += Count::new(1);
    let stream = build_completion_stream(&deps.runtime, state.tool_defs, state.history);
    let (text, tool_call) =
        match consume_stream(stream, deps.runtime.feed.tx, deps.runtime.feed.id).await {
            Err(e) => return Some(report_and_return_failed(&deps.orchestrator, e.to_string())),
            Ok(pair) => pair,
        };
    tracing::debug!(
        event = "task_turn_stream_summary",
        iteration = iterations.inner(),
        text_chars = text.as_str().len(),
        tool_call_seen = tool_call.is_some(),
    );
    accumulated.push_str(text.as_str());
    emit_feed(
        deps.runtime.feed.tx,
        deps.runtime.feed.id,
        AgentFeedOutput::MessageBreak,
    )
    .await;
    match tool_call {
        None => Some(complete_without_tool(CompletionWithoutTool {
            history: state.history,
            orchestrator: &deps.orchestrator,
            text,
            accumulated: accumulated.clone(),
        })),
        Some(call) => execute_tool_iteration(ToolIteration {
            runtime: &deps.runtime,
            history: state.history,
            call,
            text,
        })
        .await
        .err()
        .map(|error| report_and_return_failed(&deps.orchestrator, error)),
    }
}

fn report_and_return_failed(
    orchestrator: &Option<OrchestratorCorrelation>,
    reason: String,
) -> TaskSignal {
    report_failed_signal(orchestrator, OutputText::from(reason))
}

fn report_failed_signal(
    orchestrator: &Option<OrchestratorCorrelation>,
    reason: OutputText,
) -> TaskSignal {
    let signal = TaskSignal::Failed { reason };
    report_orchestrator_terminal(orchestrator, &signal);
    signal
}

fn complete_without_tool(args: CompletionWithoutTool<'_>) -> TaskSignal {
    let CompletionWithoutTool {
        history,
        orchestrator,
        text,
        accumulated,
        ..
    } = args;
    history.push(Message::assistant(text));
    tracing::debug!(
        event = "task_turn_decision",
        decision = "completed_without_tool",
        assistant_text_chars = accumulated.len(),
    );
    let signal = TaskSignal::Completed {
        output: AccumulatedText::new(accumulated),
    };
    report_orchestrator_terminal(orchestrator, &signal);
    signal
}

async fn execute_tool_iteration<L: LlmClient, T: ToolExecutor>(
    args: ToolIteration<'_, L, T>,
) -> Result<(), String> {
    let ToolIteration {
        runtime,
        history,
        call,
        text,
    } = args;
    let call_name = call.name.clone();
    let start_label = format_tool_call_line(ToolName::new(call_name.as_str()), &call.arguments);
    emit_feed(
        runtime.feed.tx,
        runtime.feed.id,
        AgentFeedOutput::ToolEventLine(start_label),
    )
    .await;
    emit_feed(
        runtime.feed.tx,
        runtime.feed.id,
        AgentFeedOutput::MessageBreak,
    )
    .await;
    history.push(Message::assistant_with_tool_calls(text, vec![call.clone()]));
    tracing::debug!(
        event = "task_tool_call_received",
        tool_name = call.name.as_str(),
        tool_id_empty = call.id.as_str().is_empty(),
        arguments_kind = tool_arguments_kind(&call.arguments),
        arguments_serialized_len = tool_arguments_len(&call.arguments),
    );

    let result = augur_domain::tools::execution::normalize_tool_execution_result(
        call.name.clone(),
        runtime.tools.execute(call.clone()).await,
    );
    tracing::debug!(
        event = "task_tool_execution_result",
        tool_name = call.name.as_str(),
        is_error = result.is_error.0,
        output_chars = result.output.as_str().len(),
        next_action = "continue_llm",
    );
    emit_feed(
        runtime.feed.tx,
        runtime.feed.id,
        AgentFeedOutput::ToolEventLine(OutputText::new(format!(
            "{} {call_name}",
            if result.is_error.0 { "✗" } else { "✓" }
        ))),
    )
    .await;
    history.push(augur_domain::tools::execution::tool_result_message(
        &call, &result,
    ));
    Ok(())
}

async fn load_spec_for_task(
    task_config: &TaskConfig,
    task_services: &TaskServices,
) -> Result<augur_domain::AgentSpec, OutputText> {
    let base = std::path::Path::new(task_services.spec_base_path.as_ref());
    let spec_path = crate::actors::openrouter_task::spec_loader::find_agent_spec_path(
        base,
        &task_config.request.agent_name,
    );
    let path = spec_path.ok_or_else(|| {
        OutputText::new(format!(
            "agent spec not found: '{}' - no matching .agent.md file in {}",
            task_config.request.agent_name.as_ref(),
            task_services.spec_base_path.as_ref(),
        ))
    })?;
    load_agent_spec(&path, task_config.request.agent_name.clone())
        .await
        .map_err(|e| OutputText::new(format!("failed to load agent spec: {e}")))
}

async fn emit_task_failed(
    target: TaskFeedTarget<'_>,
    agent_spec_name: &AgentSpecName,
    reason: OutputText,
) {
    emit_feed(
        target.tx,
        target.id,
        AgentFeedOutput::TaskFailed {
            name: AgentName::new(agent_spec_name.as_ref()),
            reason,
        },
    )
    .await;
}

async fn emit_task_started(
    target: TaskFeedTarget<'_>,
    agent_spec_name: &AgentSpecName,
    task_config: &TaskConfig,
) {
    let model_label = task_config
        .runtime
        .model_override
        .as_ref()
        .map(|m| ModelLabel::new(m.as_str()));
    emit_feed(
        target.tx,
        target.id,
        AgentFeedOutput::TaskStarted {
            name: AgentName::new(agent_spec_name.as_ref()),
            model: model_label,
        },
    )
    .await;
}

fn build_tool_defs_with_spawn<T: ToolExecutor>(
    tools: &T,
    task_config: &TaskConfig,
    spawn_tx: mpsc::Sender<SpawnAgentRequest>,
) -> Vec<ToolDefinition> {
    let spawn_tool = SpawnAgentTool::builder()
        .handle(SpawnAgentHandle(spawn_tx))
        .depth(task_config.request.depth)
        .available_agents(vec![])
        .build();
    let mut tool_defs = tools.definitions().to_vec();
    use augur_domain::tools::handler::ToolHandler as _;
    tool_defs.push(spawn_tool.definition());
    tool_defs
}

fn build_task_history(
    task_config: &TaskConfig,
    tool_defs: &[ToolDefinition],
    instructions: &augur_domain::task_types::AgentInstructions,
) -> ConversationHistory {
    let system_prompt = build_task_system_prompt(instructions, tool_defs);
    let mut history = ConversationHistory::new(system_prompt);
    history.push(Message::user(task_config.request.prompt.clone()));
    history
}

fn build_completion_stream<L: LlmClient, T: ToolExecutor>(
    runtime: &TaskLoopRuntime<'_, L, T>,
    tool_defs: &[ToolDefinition],
    history: &ConversationHistory,
) -> mpsc::Receiver<augur_domain::StreamChunk> {
    let raw = history.messages_for_request();
    let prefixed = prepend_prefix(runtime.instruction_prefix, &raw);
    let prefixed_messages_count = prefixed.len();

    // Only compact if estimated tokens exceed the auto-compact threshold.
    let estimated = estimate_request_tokens_for_compaction(&prefixed);
    let messages = if estimated > runtime.model_config.auto_compact_threshold {
        compact_messages_for_openrouter(
            prefixed,
            runtime.model_config.compaction_target,
            runtime.model_config.strip_fraction,
        )
    } else {
        prefixed
    };

    tracing::debug!(
        event = "task_llm_request_meta",
        endpoint = "openrouter",
        raw_messages_count = raw.len(),
        prefixed_messages_count,
        compacted_messages_count = messages.len(),
        tools_count = tool_defs.len(),
        estimated_tokens = ?estimated,
        auto_compact_threshold = ?runtime.model_config.auto_compact_threshold,
    );
    runtime.llm.complete_stream(
        CompletionRequest::builder()
            .endpoint(EndpointName::new("openrouter"))
            .messages(messages)
            .tools(tool_defs.to_vec())
            .maybe_model_override(runtime.model_override.clone())
            .build(),
    )
}

fn build_orchestrator_correlation(
    task_config: &TaskConfig,
    task_services: &TaskServices,
) -> Option<OrchestratorCorrelation> {
    let run_id = task_config.correlation.run_id.clone()?;
    let orchestrator = task_services.orchestrator.clone()?;
    Some(OrchestratorCorrelation {
        orchestrator,
        run_id,
    })
}

fn report_orchestrator_launch(correlation: &Option<OrchestratorCorrelation>) {
    if let Some(correlation) = correlation {
        correlation
            .orchestrator
            .transition_to_active(correlation.run_id.clone());
    }
}

fn report_orchestrator_terminal(
    correlation: &Option<OrchestratorCorrelation>,
    signal: &TaskSignal,
) {
    if let Some(correlation) = correlation {
        correlation
            .orchestrator
            .record_terminal_result(correlation.run_id.clone(), signal.clone());
    }
}

/// Consume all `StreamChunk` items from an LLM stream, emitting `StatusLine`
/// events for text tokens.
///
/// Returns `(accumulated_text, Option<ToolCall>)`. The first tool call found is
/// returned; subsequent tool calls in the same response are ignored. Returns
/// `Err` if the stream emits `StreamChunk::Error`.
///
/// # Parameters
///
/// - `rx`: the per-request mpsc receiver from `LlmClient::complete_stream`.
/// - `feed_tx`: channel for emitting `AgentFeedOutput::StatusLine` events.
async fn consume_stream(
    mut rx: mpsc::Receiver<augur_domain::StreamChunk>,
    feed_tx: &mpsc::Sender<FeedEntry>,
    feed_id: &FeedId,
) -> anyhow::Result<(OutputText, Option<ToolCall>)> {
    let mut text_buf = String::new();
    let mut tool_call: Option<ToolCall> = None;
    let mut seen_done = false;
    let mut end_reason = "channel_closed";

    while let Some(chunk) = rx.recv().await {
        match chunk {
            augur_domain::StreamChunk::Done => {
                seen_done = true;
                end_reason = "done_chunk";
                break;
            }
            augur_domain::StreamChunk::Error(e) => {
                return Err(anyhow::anyhow!("{e}"));
            }
            augur_domain::StreamChunk::Token(token) => {
                let _ = feed_tx
                    .send(FeedEntry {
                        feed_id: feed_id.clone(),
                        output: AgentFeedOutput::StatusLine(token.clone()),
                    })
                    .await;
                text_buf.push_str(token.as_str());
            }
            augur_domain::StreamChunk::ToolCall {
                id,
                name,
                arguments,
            } => {
                if tool_call.is_none() {
                    tracing::debug!(
                        event = "task_consumer_tool_call_chunk",
                        tool_name = name.as_str(),
                        tool_id_empty = id.as_str().is_empty(),
                        arguments_kind = tool_arguments_kind(&arguments),
                        arguments_serialized_len = tool_arguments_len(&arguments),
                    );
                    tool_call = Some(ToolCall {
                        id,
                        name,
                        arguments,
                    });
                } else {
                    tracing::debug!(
                        event = "task_consumer_additional_tool_call_ignored",
                        tool_name = name.as_str(),
                    );
                }
            }
            augur_domain::StreamChunk::Usage(_) | augur_domain::StreamChunk::RateLimitRetry(_) => {}
        }
    }
    tracing::debug!(
        event = "task_consumer_stream_end",
        end_reason,
        seen_done,
        text_chars = text_buf.len(),
        tool_call_seen = tool_call.is_some(),
    );

    Ok((OutputText::from(text_buf), tool_call))
}

fn tool_arguments_kind(arguments: &serde_json::Value) -> &'static str {
    match arguments {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn tool_arguments_len(arguments: &serde_json::Value) -> usize {
    serde_json::to_string(arguments)
        .map(|s| s.len())
        .unwrap_or(0)
}

/// Fire-and-forget send to the feed channel.
///
/// Uses `send().await` so back-pressure is respected. Errors are silently
/// discarded - if the TUI has stopped listening the task should still complete.
fn task_feed_id(task_config: &TaskConfig) -> FeedId {
    task_config
        .correlation
        .run_id
        .as_ref()
        .map(|run_id| {
            FeedId::Agent(augur_domain::string_newtypes::ToolCallId::from(
                run_id.as_ref(),
            ))
        })
        .unwrap_or_else(|| {
            FeedId::Agent(augur_domain::string_newtypes::ToolCallId::from(
                uuid::Uuid::new_v4().to_string(),
            ))
        })
}

async fn emit_feed(feed_tx: &mpsc::Sender<FeedEntry>, feed_id: &FeedId, event: AgentFeedOutput) {
    let _ = feed_tx
        .send(FeedEntry {
            feed_id: feed_id.clone(),
            output: event,
        })
        .await;
}
