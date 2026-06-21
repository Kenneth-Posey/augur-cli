//! Deterministic lifecycle/task-arg core for the OpenRouter orchestrator actor.

use super::openrouter_orchestrator_actor::{
    AwaitWaiter, BuildOpenRouterTaskArgsInput, OpenRouterOrchestratorCommand,
    OpenRouterOrchestratorState, QueuedSpawn, RunSchedulingState,
};
use super::openrouter_orchestrator_ops::{
    consume_terminal_result, record_terminal_result, status_snapshot, transition_to_active,
    StatusSnapshotInput, TerminalResultRecord, TransitionToActive,
};
use crate::actors::llm::handle::LlmHandle;
use crate::actors::openrouter_task::openrouter_task_actor as actor;
use crate::actors::openrouter_task::openrouter_task_actor::{
    OpenRouterTaskArgs, TaskConfig, TaskCorrelation, TaskRequestSpec, TaskRuntimeOptions,
    TaskServices,
};
use augur_domain::actors::tool::InlineToolExecutor;
use augur_domain::task_types::{
    AwaitRunResult, InstructionPrefix, RepoRoot, SpawnAgentAck, SpawnAgentRequest,
    SpawnDispatchStatus, TaskDispatchState, TaskQueueSnapshot, TaskRunId,
};
use augur_domain::Message;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Monotonic generation counter that invalidates stale orchestrator session work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SessionGeneration(
    /// Monotonic generation value.
    pub u64,
);

/// Main command-processing loop for the orchestrator assistant core.
pub(super) async fn run_loop(
    mut cmd_rx: mpsc::Receiver<OpenRouterOrchestratorCommand>,
    mut state: OpenRouterOrchestratorState,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        if matches!(handle_command(&mut state, cmd), CommandHandling::BreakLoop) {
            break;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandHandling {
    ContinueLoop,
    BreakLoop,
}

fn handle_command(
    state: &mut OpenRouterOrchestratorState,
    command: OpenRouterOrchestratorCommand,
) -> CommandHandling {
    if matches!(command, OpenRouterOrchestratorCommand::Shutdown) {
        clear_scheduling_state(&mut state.scheduling);
        return CommandHandling::BreakLoop;
    }
    handle_non_shutdown_command(state, command);
    CommandHandling::ContinueLoop
}

fn handle_non_shutdown_command(
    state: &mut OpenRouterOrchestratorState,
    command: OpenRouterOrchestratorCommand,
) {
    match command {
        OpenRouterOrchestratorCommand::EnqueueSpawn {
            request,
            model_override,
        } => handle_enqueue_spawn(state, request, model_override),
        lifecycle @ OpenRouterOrchestratorCommand::TransitionToActive { .. }
        | lifecycle @ OpenRouterOrchestratorCommand::TerminalResult { .. } => {
            handle_lifecycle_command(state, lifecycle)
        }
        await_or_query @ OpenRouterOrchestratorCommand::AwaitRun { .. }
        | await_or_query @ OpenRouterOrchestratorCommand::AwaitAny { .. }
        | await_or_query @ OpenRouterOrchestratorCommand::QueryStatus { .. } => {
            handle_await_or_query_command(&mut state.scheduling, await_or_query)
        }
        session @ OpenRouterOrchestratorCommand::ResetSession
        | session @ OpenRouterOrchestratorCommand::Shutdown => {
            handle_session_or_shutdown_command(state, session)
        }
    }
}

fn handle_lifecycle_command(
    state: &mut OpenRouterOrchestratorState,
    command: OpenRouterOrchestratorCommand,
) {
    match command {
        OpenRouterOrchestratorCommand::TransitionToActive { run_id } => {
            handle_transition_to_active(&mut state.scheduling, run_id)
        }
        OpenRouterOrchestratorCommand::TerminalResult { run_id, signal } => {
            handle_terminal_result(state, run_id, signal)
        }
        _ => {}
    }
}

fn handle_await_or_query_command(
    scheduling: &mut RunSchedulingState,
    command: OpenRouterOrchestratorCommand,
) {
    match command {
        OpenRouterOrchestratorCommand::AwaitRun { run_id, reply_tx } => {
            handle_await_run(scheduling, run_id, reply_tx)
        }
        OpenRouterOrchestratorCommand::AwaitAny { run_ids, reply_tx } => {
            handle_await_any(scheduling, run_ids, reply_tx)
        }
        OpenRouterOrchestratorCommand::QueryStatus { reply_tx } => {
            handle_query_status(scheduling, reply_tx)
        }
        _ => {}
    }
}

fn handle_session_or_shutdown_command(
    state: &mut OpenRouterOrchestratorState,
    command: OpenRouterOrchestratorCommand,
) {
    if matches!(command, OpenRouterOrchestratorCommand::ResetSession) {
        handle_reset_session(state);
    }
}

fn handle_enqueue_spawn(
    state: &mut OpenRouterOrchestratorState,
    request: SpawnAgentRequest,
    model_override: Option<augur_domain::ModelId>,
) {
    enqueue_spawn(state, request, model_override);
    dispatch_queued_runs(state);
}

fn handle_transition_to_active(scheduling: &mut RunSchedulingState, run_id: TaskRunId) {
    if should_accept_lifecycle_event(scheduling, &run_id) {
        let _transition_outcome =
            transition_to_active(&mut scheduling.ledger, TransitionToActive { run_id });
    }
}

fn handle_terminal_result(
    state: &mut OpenRouterOrchestratorState,
    run_id: TaskRunId,
    signal: augur_domain::task_types::TaskSignal,
) {
    if !should_accept_lifecycle_event(&state.scheduling, &run_id) {
        return;
    }
    let terminal_outcome = record_terminal_result(
        &mut state.scheduling.ledger,
        TerminalResultRecord {
            run_id: run_id.clone(),
            signal,
        },
    );
    let _ = terminal_outcome;
    state.scheduling.active_joins.remove(&run_id);
    satisfy_waiters_for_run(&mut state.scheduling, run_id);
    dispatch_queued_runs(state);
}

fn handle_await_run(
    scheduling: &mut RunSchedulingState,
    run_id: TaskRunId,
    reply_tx: tokio::sync::oneshot::Sender<AwaitRunResult>,
) {
    let result = consume_or_defer_await(scheduling, vec![run_id], reply_tx);
    send_await_result(result);
}

fn handle_await_any(
    scheduling: &mut RunSchedulingState,
    run_ids: Vec<TaskRunId>,
    reply_tx: tokio::sync::oneshot::Sender<AwaitRunResult>,
) {
    let result = consume_or_defer_await(scheduling, run_ids, reply_tx);
    send_await_result(result);
}

fn send_await_result(
    result: Option<(tokio::sync::oneshot::Sender<AwaitRunResult>, AwaitRunResult)>,
) {
    if let Some((reply_tx, await_result)) = result {
        let _ = reply_tx.send(await_result);
    }
}

fn handle_query_status(
    scheduling: &RunSchedulingState,
    reply_tx: tokio::sync::oneshot::Sender<augur_domain::task_types::TaskRunStatusSnapshot>,
) {
    let snapshot = status_snapshot(
        &scheduling.ledger,
        StatusSnapshotInput {
            max_parallel_workers: scheduling.max_parallel_workers,
            queued_runs: scheduling.queue.len(),
        },
    );
    let _ = reply_tx.send(snapshot);
}

fn handle_reset_session(state: &mut OpenRouterOrchestratorState) {
    state.session_generation = state.session_generation.saturating_add(1);
    clear_scheduling_state(&mut state.scheduling);
}

fn clear_scheduling_state(scheduling: &mut RunSchedulingState) {
    abort_active_joins(&mut scheduling.active_joins);
    scheduling.ledger.pending_runs.clear();
    scheduling.ledger.active_runs.clear();
    scheduling.ledger.terminal_results.clear();
    scheduling.ledger.consumed_runs.clear();
    scheduling.queue.clear();
    let mut waiters = VecDeque::new();
    std::mem::swap(&mut waiters, &mut scheduling.await_waiters);
    for waiter in waiters {
        let run_id = waiter
            .run_ids
            .first()
            .cloned()
            .unwrap_or_else(|| TaskRunId::new("unknown"));
        let _ = waiter.reply_tx.send(AwaitRunResult::UnknownRun { run_id });
    }
}

fn abort_active_joins(active_joins: &mut HashMap<TaskRunId, tokio::task::JoinHandle<()>>) {
    let mut joins = HashMap::new();
    std::mem::swap(&mut joins, active_joins);
    for (_run_id, join) in joins {
        join.abort();
    }
}

fn should_accept_lifecycle_event(scheduling: &RunSchedulingState, run_id: &TaskRunId) -> bool {
    scheduling.ledger.pending_runs.contains(run_id)
        || scheduling.ledger.active_runs.contains(run_id)
        || scheduling.active_joins.contains_key(run_id)
}

fn enqueue_spawn(
    state: &mut OpenRouterOrchestratorState,
    request: SpawnAgentRequest,
    model_override: Option<augur_domain::ModelId>,
) {
    state
        .scheduling
        .ledger
        .pending_runs
        .insert(request.run_id.clone());
    let queue_position = state.scheduling.queue.len();
    let dispatch_state = if state.scheduling.active_joins.len()
        < state.scheduling.max_parallel_workers
        && queue_position == 0
    {
        TaskDispatchState::Dispatched
    } else {
        TaskDispatchState::Queued {
            position: queue_position,
        }
    };
    let status = SpawnDispatchStatus::builder()
        .run_id(request.run_id.clone())
        .dispatch_state(dispatch_state.clone())
        .queue_snapshot(
            TaskQueueSnapshot::builder()
                .max_parallel_workers(state.scheduling.max_parallel_workers)
                .active_runs(state.scheduling.active_joins.len())
                .queued_runs(queued_runs_snapshot(&dispatch_state, queue_position))
                .build(),
        )
        .build();
    let SpawnAgentRequest {
        agent_name,
        prompt,
        depth,
        run_id,
        channels,
    } = request;
    let _ = channels.ack_tx.send(SpawnAgentAck::Completed { status });
    state.scheduling.queue.push_back(
        QueuedSpawn::builder()
            .request(
                super::openrouter_orchestrator_actor::QueuedSpawnRequest::builder()
                    .agent_name(agent_name)
                    .prompt(prompt)
                    .depth(depth)
                    .run_id(run_id)
                    .terminal_tx(channels.terminal_tx)
                    .build(),
            )
            .maybe_model_override(model_override)
            .build(),
    );
}

fn queued_runs_snapshot(dispatch_state: &TaskDispatchState, queue_position: usize) -> usize {
    match dispatch_state {
        TaskDispatchState::Dispatched => queue_position,
        TaskDispatchState::Queued { .. } => queue_position + 1,
    }
}

fn dispatch_queued_runs(state: &mut OpenRouterOrchestratorState) {
    while state.scheduling.active_joins.len() < state.scheduling.max_parallel_workers {
        let Some(queued) = state.scheduling.queue.pop_front() else {
            break;
        };
        let run_id = queued.request.run_id.clone();
        let openrouter_args = build_openrouter_task_args(
            BuildOpenRouterTaskArgsInput::builder()
                .args(state.args.clone())
                .orchestrator(state.self_handle.clone())
                .queued_spawn(queued)
                .session_generation(state.session_generation)
                .build(),
        );
        let (join, _task_handle) = actor::spawn(openrouter_args);
        let _transition_outcome = transition_to_active(
            &mut state.scheduling.ledger,
            TransitionToActive {
                run_id: run_id.clone(),
            },
        );
        state.scheduling.active_joins.insert(run_id, join);
    }
}

fn consume_or_defer_await(
    scheduling: &mut RunSchedulingState,
    run_ids: Vec<TaskRunId>,
    reply_tx: tokio::sync::oneshot::Sender<AwaitRunResult>,
) -> Option<(tokio::sync::oneshot::Sender<AwaitRunResult>, AwaitRunResult)> {
    if let Some(empty_result) = empty_run_ids_result(&run_ids) {
        return Some((reply_tx, empty_result));
    }
    if let Some(immediate) = consume_immediate_result(scheduling, &run_ids) {
        return Some((reply_tx, immediate));
    }
    if should_defer_await(scheduling, &run_ids) {
        defer_await(scheduling, run_ids, reply_tx);
        return None;
    }
    Some((reply_tx, unknown_run_result(&run_ids)))
}

fn empty_run_ids_result(run_ids: &[TaskRunId]) -> Option<AwaitRunResult> {
    if run_ids.is_empty() {
        return Some(AwaitRunResult::UnknownRun {
            run_id: TaskRunId::new(""),
        });
    }
    None
}

fn consume_immediate_result(
    scheduling: &mut RunSchedulingState,
    run_ids: &[TaskRunId],
) -> Option<AwaitRunResult> {
    run_ids.iter().find_map(|run_id| {
        let immediate = consume_terminal_result(&mut scheduling.ledger, run_id.clone());
        matches!(
            immediate,
            AwaitRunResult::ConsumedTerminal { .. } | AwaitRunResult::AlreadyConsumed { .. }
        )
        .then_some(immediate)
    })
}

fn should_defer_await(scheduling: &RunSchedulingState, run_ids: &[TaskRunId]) -> bool {
    run_ids.iter().any(|run_id| {
        scheduling.ledger.pending_runs.contains(run_id)
            || scheduling.ledger.active_runs.contains(run_id)
    })
}

fn defer_await(
    scheduling: &mut RunSchedulingState,
    run_ids: Vec<TaskRunId>,
    reply_tx: tokio::sync::oneshot::Sender<AwaitRunResult>,
) {
    scheduling.await_waiters.push_back(
        AwaitWaiter::builder()
            .run_ids(run_ids)
            .reply_tx(reply_tx)
            .build(),
    );
}

fn unknown_run_result(run_ids: &[TaskRunId]) -> AwaitRunResult {
    AwaitRunResult::UnknownRun {
        run_id: run_ids[0].clone(),
    }
}

fn satisfy_waiters_for_run(scheduling: &mut RunSchedulingState, run_id: TaskRunId) {
    let mut retained = VecDeque::new();
    let mut waiters = VecDeque::new();
    std::mem::swap(&mut waiters, &mut scheduling.await_waiters);
    while let Some(waiter) = waiters.pop_front() {
        if waiter.run_ids.iter().any(|candidate| candidate == &run_id) {
            let result = consume_terminal_result(&mut scheduling.ledger, run_id.clone());
            let _ = waiter.reply_tx.send(result);
        } else {
            retained.push_back(waiter);
        }
    }
    scheduling.await_waiters = retained;
}

/// Build task arguments for one OpenRouter task spawn request.
pub(super) fn build_openrouter_task_args(
    input: BuildOpenRouterTaskArgsInput,
) -> OpenRouterTaskArgs<LlmHandle, InlineToolExecutor> {
    let BuildOpenRouterTaskArgsInput {
        args,
        orchestrator,
        queued_spawn,
        session_generation,
    } = input;
    let super::openrouter_orchestrator_actor::QueuedSpawnRequest {
        agent_name,
        prompt,
        depth,
        run_id,
        terminal_tx,
    } = queued_spawn.request;
    let request = TaskRequestSpec::builder()
        .agent_name(agent_name)
        .prompt(prompt)
        .depth(depth)
        .build();
    let correlation = TaskCorrelation::builder()
        .signal_tx(terminal_tx)
        .maybe_run_id(Some(run_id))
        .build();
    let task_config = build_task_config(
        BuildTaskConfigArgs::builder()
            .orchestrator_args(&args)
            .request(request)
            .correlation(correlation)
            .maybe_model_override(queued_spawn.model_override)
            .build(),
    );
    let task_services =
        build_task_services(&args, orchestrator, SessionGeneration(session_generation));
    OpenRouterTaskArgs::builder()
        .llm(args.runtime.llm.clone())
        .tools(args.runtime.tool_executor.clone())
        .task_config(task_config)
        .task_services(task_services)
        .build()
}

#[derive(bon::Builder)]
struct BuildTaskConfigArgs<'a> {
    orchestrator_args: &'a super::openrouter_orchestrator_actor::OpenRouterOrchestratorArgs,
    request: TaskRequestSpec,
    correlation: TaskCorrelation,
    model_override: Option<augur_domain::ModelId>,
}

fn build_task_config(args: BuildTaskConfigArgs<'_>) -> TaskConfig {
    let BuildTaskConfigArgs {
        orchestrator_args,
        request,
        correlation,
        model_override,
    } = args;
    TaskConfig::builder()
        .request(request)
        .runtime(
            TaskRuntimeOptions::builder()
                .maybe_model_override(
                    model_override
                        .or_else(|| orchestrator_args.runtime.active_model.current_model()),
                )
                .build(),
        )
        .correlation(correlation)
        .build()
}

fn build_task_services(
    args: &super::openrouter_orchestrator_actor::OpenRouterOrchestratorArgs,
    orchestrator: crate::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle,
    session_generation: SessionGeneration,
) -> TaskServices {
    TaskServices::builder()
        .feed_tx(args.io.feed_tx.clone())
        .instruction_prefix(instruction_prefix_with_session_generation(
            args.config.instruction_prefix.clone(),
            session_generation,
        ))
        .spec_base_path(RepoRoot::new(format!(
            "{}/.github/agents",
            args.config.repo_root.as_ref()
        )))
        .maybe_orchestrator(Some(orchestrator))
        .build()
}

/// Extend the instruction prefix with a session-generation marker message.
pub(super) fn instruction_prefix_with_session_generation(
    instruction_prefix: Arc<InstructionPrefix>,
    session_generation: SessionGeneration,
) -> Arc<InstructionPrefix> {
    let mut contextual_messages = instruction_prefix.0.clone();
    contextual_messages.push(Message::system(format!(
        "openrouter_session_generation={}",
        session_generation.0
    )));
    Arc::new(InstructionPrefix(contextual_messages))
}
