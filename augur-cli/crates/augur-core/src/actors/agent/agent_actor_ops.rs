//! Private helper operations for the agent actor shell.

use super::agent_actor::{
    AgentCommand, AgentRunState, AgentSpawnArgs, FinalizeTurnArgs, FinalizeTurnState,
    RestoreHistoryArgs, RunPipes, SubmitCmdInput, SubmitPayload, SubmitTurnArgs, SubmitTurnRequest,
};
use super::agent_ops::{
    AgentOutput, DEFAULT_MAX_ITERATIONS, TurnConfig, build_extended_system_prompt,
    build_message_records, make_error_annotation, merge_with_error_annotations,
};
use super::history::ConversationHistory;
use crate::actors::cache::handle::CacheHandle;
use augur_domain::domain::channels::AGENT_OUTPUT_CAPACITY;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::string_newtypes::ModelId;
use augur_domain::domain::{
    CancelSignal, ContextUsageStats, LlmClient, Message, OutputText, StringNewtype, TokenCount,
    ToolExecutor,
};
use augur_domain::persistence::types::MessageRecord;
use tokio::sync::broadcast;

/// Resolve the output broadcast sender, creating one when none is supplied.
pub(super) fn resolve_output_tx(
    configured: Option<broadcast::Sender<AgentOutput>>,
) -> broadcast::Sender<AgentOutput> {
    configured.unwrap_or_else(|| {
        let (tx, _) = broadcast::channel(*AGENT_OUTPUT_CAPACITY);
        tx
    })
}

/// Convert a model id from `SetModel` into an optional selected-model override.
pub(super) fn normalize_selected_model(model_id: &ModelId) -> Option<ModelId> {
    if model_id.as_str().is_empty() {
        None
    } else {
        Some(model_id.clone())
    }
}

/// Processes a single `Submit` command and finalises all side effects.
pub(super) async fn handle_submit_cmd<L: LlmClient, T: ToolExecutor>(
    pipes: &mut RunPipes,
    args: &AgentSpawnArgs<L, T>,
    cmd: SubmitCmdInput<'_>,
) {
    let SubmitCmdInput { run_state, payload } = cmd;
    let len_before = run_state.history.len().inner();
    let turn_result = run_submit_turn(
        SubmitTurnArgs::builder()
            .pipes(pipes)
            .actor_args(args)
            .history(&mut run_state.history)
            .request(
                SubmitTurnRequest::builder()
                    .prompt(payload.prompt)
                    .endpoint(payload.endpoint.clone())
                    .maybe_model_override(run_state.selected_model.clone())
                    .build(),
            )
            .build(),
    )
    .await;
    let turn_completed_without_error = turn_result.error.is_none();
    finalize_turn(
        FinalizeTurnArgs::builder()
            .actor_args(args)
            .endpoint(payload.endpoint)
            .state(
                FinalizeTurnState::builder()
                    .history(&mut run_state.history)
                    .error_annotations(&mut run_state.error_annotations)
                    .len_before(len_before)
                    .turn_result(turn_result)
                    .build(),
            )
            .build(),
    )
    .await;
    if turn_completed_without_error {
        let _ = pipes.output_tx.send(AgentOutput::Done);
    }
}

/// Reset conversation history to a fresh state, clearing all messages
/// and error annotations while keeping the system prompt.
fn clear_history(
    history: &mut ConversationHistory,
    error_annotations: &mut Vec<(augur_domain::domain::newtypes::Count, MessageRecord)>,
    extended_prompt: &augur_domain::domain::OutputText,
) {
    *history = ConversationHistory::new(extended_prompt.clone());
    error_annotations.clear();
}

/// Compact history using the agent's message compactor, if configured.
///
/// Uses the `message_compactor` from AgentExtensions to compact the current
/// conversation history. Replaces both the conversation and OpenRouter context
/// messages with the compacted result. Emits a system message notification.
fn compact_history(
    history: &mut ConversationHistory,
    output_tx: &broadcast::Sender<AgentOutput>,
    compactor: &augur_domain::domain::task_types::MessageCompactor,
    model_id: Option<ModelId>,
) {
    let messages = history.messages_for_request();
    let compacted = compactor(messages, model_id);
    let len_before = history.len().inner();
    history.set_messages(compacted);
    let len_after = history.len().inner();
    let _ = output_tx.send(AgentOutput::SystemMessage(OutputText::new(format!(
        "[system] context compacted: {len_before} -> {len_after} messages",
    ))));
}

/// Main actor receive loop.
pub(super) async fn run_loop<L: LlmClient, T: ToolExecutor>(
    pipes: &mut RunPipes,
    args: &AgentSpawnArgs<L, T>,
) {
    let tool_defs = args.tools.definitions().to_vec();
    let extended_prompt = build_extended_system_prompt(&args.config.system_prompt, &tool_defs);
    let mut run_state = AgentRunState {
        history: ConversationHistory::new(extended_prompt.clone()),
        error_annotations: Vec::new(),
        selected_model: None,
        last_endpoint: None,
    };

    while let Some(cmd) = pipes.cmd_rx.recv().await {
        match cmd {
            AgentCommand::Shutdown => break,
            AgentCommand::RestoreSession(records) => {
                let openrouter_context_records =
                    args.services.persistence.openrouter_context_history();
                restore_history(
                    RestoreHistoryArgs::builder()
                        .history(&mut run_state.history)
                        .error_annotations(&mut run_state.error_annotations)
                        .extended_prompt(&extended_prompt)
                        .records(records)
                        .maybe_openrouter_context_records(openrouter_context_records)
                        .build(),
                );
            }
            AgentCommand::SnapshotHistory { reply_tx } => {
                let _ = reply_tx.send(run_state.history.messages().to_vec());
            }
            AgentCommand::ClearHistory => {
                tracing::info!("agent.clear_history: resetting conversation history");
                args.services.persistence.clear_openrouter_context_history();
                clear_history(
                    &mut run_state.history,
                    &mut run_state.error_annotations,
                    &extended_prompt,
                );
            }
            AgentCommand::Compact => {
                if let Some(ref compactor) = args.runtime.extensions.message_compactor {
                    tracing::info!("agent.compact: applying message compactor");
                    compact_history(
                        &mut run_state.history,
                        &pipes.output_tx,
                        compactor,
                        run_state.selected_model.clone(),
                    );
                } else {
                    tracing::info!("agent.compact: no compactor configured, no-op");
                    let _ = pipes
                        .output_tx
                        .send(AgentOutput::SystemMessage(OutputText::new(
                            "[system] no compactor configured for this endpoint",
                        )));
                }
            }
            AgentCommand::SetModel(model_id) => {
                run_state.selected_model = normalize_selected_model(&model_id);
                let _ = pipes
                    .output_tx
                    .send(AgentOutput::ActiveModelChanged(model_id));
            }
            AgentCommand::GetState { reply_tx } => {
                let state = super::agent_actor::AgentState {
                    last_endpoint: run_state.last_endpoint.clone(),
                    selected_model: run_state.selected_model.clone(),
                };
                let _ = reply_tx.send(state);
            }
            AgentCommand::Submit { prompt, endpoint } => {
                run_state.last_endpoint = Some(endpoint.clone());
                let payload = SubmitPayload { prompt, endpoint };
                handle_submit_cmd(
                    pipes,
                    args,
                    SubmitCmdInput {
                        run_state: &mut run_state,
                        payload,
                    },
                )
                .await;
            }
        }
    }
}

/// Restore in-memory conversation history from persisted message records.
pub(super) fn restore_history(args: RestoreHistoryArgs<'_>) {
    let RestoreHistoryArgs {
        history,
        error_annotations,
        extended_prompt,
        records,
        openrouter_context_records,
    } = args;
    *history = ConversationHistory::from_messages_with_openrouter_context(
        extended_prompt.clone(),
        records,
        openrouter_context_records,
    );
    error_annotations.clear();
}

/// Execute one submit turn and clear cancellation state after completion.
pub(super) async fn run_submit_turn<L: LlmClient, T: ToolExecutor>(
    args: SubmitTurnArgs<'_, L, T>,
) -> super::assistant_core::TurnResult {
    let SubmitTurnArgs {
        pipes,
        actor_args,
        history,
        request,
    } = args;
    let SubmitTurnRequest {
        prompt,
        endpoint,
        model_override,
    } = request;
    let _ = pipes.cancel_rx.borrow_and_update();
    history.push(Message::user(prompt));
    let turn_cfg = TurnConfig {
        max_iterations: DEFAULT_MAX_ITERATIONS,
        endpoint,
        model_override,
        app_config: actor_args.runtime.app_config.clone(),
        max_context_length: actor_args.runtime.max_context_length,
        request_cap_threshold: actor_args.runtime.request_cap_threshold,
    };
    let actors_cache = actor_args
        .runtime
        .extensions
        .cache
        .as_ref()
        .and_then(|h| h.0.downcast_ref::<CacheHandle>());
    let prefix = actor_args.runtime.extensions.instruction_prefix.as_deref();
    let ext = super::assistant_core::TurnExtensions {
        cache: actors_cache,
        instruction_prefix: prefix,
    };
    let ctx = super::assistant_core::TurnContext::builder()
        .llm(&actor_args.llm)
        .tools(&actor_args.tools)
        .output_tx(&pipes.output_tx)
        .cancel_rx(&mut pipes.cancel_rx)
        .ext(ext)
        .build();
    let turn_result = super::agent_actor::process_turn(history, ctx, turn_cfg).await;
    let _ = pipes.cancel_tx.send(CancelSignal::Clear);
    turn_result
}

/// Persist turn output and publish history/token side effects for new messages.
pub(super) async fn finalize_turn<L: LlmClient, T: ToolExecutor>(args: FinalizeTurnArgs<'_, L, T>) {
    let FinalizeTurnArgs {
        actor_args,
        endpoint,
        state,
    } = args;
    let FinalizeTurnState {
        history,
        error_annotations,
        len_before,
        turn_result,
    } = state;
    if let Some(error) = turn_result.error {
        error_annotations.push((
            augur_domain::domain::newtypes::Count::new(history.len().inner()),
            make_error_annotation(error),
        ));
    }
    let base_records = build_message_records(history.messages(), turn_result.usage.clone());
    let records = merge_with_error_annotations(base_records, error_annotations);
    if super::assistant_core::is_openrouter_endpoint(&endpoint, &actor_args.runtime.app_config).0 {
        actor_args
            .services
            .persistence
            .set_openrouter_context_history(history.openrouter_context_messages().to_vec());
    } else {
        actor_args
            .services
            .persistence
            .clear_openrouter_context_history();
    }
    actor_args
        .services
        .persistence
        .save_turn(endpoint.clone(), records)
        .await;
    let new_messages = history.messages()[len_before..].to_vec();
    for msg in &new_messages {
        match msg.role {
            augur_domain::domain::types::Role::User | augur_domain::domain::types::Role::Tool => {
                actor_args.services.history_adapter.record_user(msg.clone());
            }
            _ => {
                actor_args.services.history_adapter.record_llm(msg.clone());
            }
        }
    }
    if let Some(ref usage) = turn_result.usage {
        actor_args
            .services
            .token_tracker
            .record_usage(usage.clone());
        let stats = ContextUsageStats {
            current_tokens: usage.tokens_in,
            token_limit: TokenCount::new(0),
            messages_length: turn_result.messages_len,
        };
        actor_args.services.token_tracker.record_context(stats);
    }
}
