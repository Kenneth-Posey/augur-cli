//! Infrastructure dispatch and artifact helpers for the deterministic orchestrator actor.

use super::super::artifact_store::{ArtifactUpdate, StepArtifactResolver};
use super::super::background_dispatch::DeterministicAgentDispatcher;
use super::super::commands::DeterministicOrchestratorCmd;
use super::{
    AppliedDecision, CompletionForwarderArgs, DeterministicOrchestratorRunState,
    EvaluatorDispatchFailure, RuntimePorts,
};
use crate::domain::deterministic_orchestrator::{
    DeterministicOrchestratorEvent, FailureOrigin, NormalizedSignal, StepExecutionRecord,
    WorkflowStep,
};
use crate::domain::deterministic_orchestrator_ops::{DispatchRequestKind, WorkflowDispatchRequest};
use augur_domain::domain::WorkflowStepId;
use tokio::sync::broadcast;

/// Sends a dispatch request and spawns a completion forwarder for the result.
pub async fn dispatch_request(
    ports: &RuntimePorts,
    artifact_store: StepArtifactResolver,
    request: WorkflowDispatchRequest,
) {
    let dispatcher = build_dispatcher(ports);
    let dispatch_kind = request.kind.clone();
    let dispatch_result = dispatch_to_agent(&dispatcher, &request, &dispatch_kind).await;

    match dispatch_result {
        Ok(ticket) => spawn_completion_forwarder(
            ports,
            CompletionForwarderArgs {
                dispatcher,
                ticket,
                artifact_store,
                request,
            },
        ),
        Err(error) => {
            tracing::warn!(
                step_id = %request.step_id,
                error = %error,
                dispatch_kind = ?request.kind,
                "deterministic agent dispatch failed"
            );
            let _ = ports.cmd_tx.send(agent_execution_failed_cmd(request)).await;
        }
    }
}

fn build_dispatcher(ports: &RuntimePorts) -> DeterministicAgentDispatcher {
    match &ports.agent_feed_tx {
        Some(tx) => {
            DeterministicAgentDispatcher::new_with_feed(ports.dispatch_runtime.clone(), tx.clone())
        }
        None => DeterministicAgentDispatcher::new(ports.dispatch_runtime.clone()),
    }
}

async fn dispatch_to_agent(
    dispatcher: &DeterministicAgentDispatcher,
    request: &WorkflowDispatchRequest,
    dispatch_kind: &DispatchRequestKind,
) -> Result<
    super::super::background_dispatch::AgentDispatchTicket,
    super::super::background_dispatch::DispatchError,
> {
    match dispatch_kind {
        DispatchRequestKind::Worker => dispatcher.dispatch_worker_agent(request).await,
        DispatchRequestKind::Evaluator => dispatcher.dispatch_evaluator_agent(request).await,
    }
}

fn agent_execution_failed_cmd(request: WorkflowDispatchRequest) -> DeterministicOrchestratorCmd {
    DeterministicOrchestratorCmd::AgentExecutionFailed {
        step_id: request.step_id,
        kind: request.kind,
    }
}

/// Spawns an async task that awaits agent completion and forwards the result back as a command.
pub fn spawn_completion_forwarder(ports: &RuntimePorts, args: CompletionForwarderArgs) {
    let cmd_tx = ports.cmd_tx.clone();
    let dispatch_kind = args.ticket.kind.clone();
    let step_id = args.ticket.step_id.clone();

    tokio::spawn(async move {
        let (signal, evaluator_output) =
            match args.dispatcher.await_agent_completion(args.ticket).await {
                Ok(result) => result,
                Err(error) => {
                    tracing::warn!(
                        step_id = %step_id,
                        error = %error,
                        dispatch_kind = ?dispatch_kind,
                        "agent completion await failed"
                    );
                    let _ = cmd_tx
                        .send(DeterministicOrchestratorCmd::AgentExecutionFailed {
                            step_id,
                            kind: dispatch_kind,
                        })
                        .await;
                    return;
                }
            };
        let artifact_updates = args
            .artifact_store
            .capture_artifact_updates(&args.request.artifacts.created_artifacts);

        let command = match dispatch_kind {
            DispatchRequestKind::Worker => DeterministicOrchestratorCmd::WorkerCompleted {
                step_id,
                signal,
                artifact_updates,
            },
            DispatchRequestKind::Evaluator => DeterministicOrchestratorCmd::EvaluatorCompleted {
                step_id,
                signal,
                artifact_updates,
                evaluator_output,
            },
        };

        let _ = cmd_tx.send(command).await;
    });
}

/// Handles an infrastructure failure on the worker dispatch path.
pub async fn handle_worker_dispatch_failure(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step: WorkflowStep,
) {
    let execution = super::worker_execution_record(&step, NormalizedSignal::Hold);
    super::handle_step_evaluation(
        state,
        ports,
        super::EvaluatedStep {
            step,
            execution,
            transition_signal: NormalizedSignal::Hold,
            failure_origin: FailureOrigin::Infrastructure,
            artifact_updates: vec![],
        },
    )
    .await;
}

/// Handles an infrastructure failure on the evaluator dispatch path.
pub async fn handle_evaluator_dispatch_failure(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    failure: EvaluatorDispatchFailure,
) {
    let Some(worker_execution) = state.pending_worker.clone() else {
        emit_halted(&ports.event_tx, failure.step_id);
        state.run_state.current_step_id = None;
        state.pending_worker = None;
        state.run_state.pending_failure = None;
        return;
    };
    if worker_execution.execution.step_id != failure.step_id {
        return;
    }

    let execution = super::evaluator_execution_record(
        &worker_execution.execution,
        NormalizedSignal::Hold,
        None,
    );
    super::handle_step_evaluation(
        state,
        ports,
        super::EvaluatedStep {
            step: failure.step,
            execution,
            transition_signal: NormalizedSignal::Hold,
            failure_origin: FailureOrigin::Infrastructure,
            artifact_updates: worker_execution.artifact_updates,
        },
    )
    .await;
}

/// Deduplicates and merges two artifact update lists, with later updates overwriting earlier ones.
pub fn merge_artifact_updates(
    earlier_updates: Vec<ArtifactUpdate>,
    later_updates: Vec<ArtifactUpdate>,
) -> Vec<ArtifactUpdate> {
    let mut merged = earlier_updates;

    for update in later_updates {
        if let Some(index) = merged
            .iter()
            .position(|candidate| candidate.artifact == update.artifact)
        {
            merged[index] = update;
        } else {
            merged.push(update);
        }
    }

    merged
}

/// Applies artifact updates to the step artifact store.
pub fn apply_artifact_updates(
    state: &DeterministicOrchestratorRunState,
    execution: &StepExecutionRecord,
    updates: &[ArtifactUpdate],
) {
    if updates.is_empty() {
        return;
    }

    if let Err(error) = state
        .artifact_store
        .apply_in_place_artifact_updates(execution, updates)
    {
        tracing::warn!(
            step_id = %execution.step_id,
            error = %error,
            "failed to apply deterministic artifact updates"
        );
    }
}

/// Annotates the failure decision on the last step execution record.
pub fn annotate_last_failure_decision(
    state: &mut DeterministicOrchestratorRunState,
    applied: &AppliedDecision,
) {
    let Some(last_record) = state.run_state.prior_steps.last_mut() else {
        return;
    };
    if last_record.step_id != applied.step_id {
        return;
    }
    last_record.remediation_record.failure_decision = applied.decision.clone();
}

/// Emits a broadcast event, ignoring send failures caused by no active receivers.
pub fn emit(
    event_tx: &broadcast::Sender<DeterministicOrchestratorEvent>,
    event: DeterministicOrchestratorEvent,
) {
    let _ = event_tx.send(event);
}

/// Emits a `Halted` event for the given step.
pub fn emit_halted(
    event_tx: &broadcast::Sender<DeterministicOrchestratorEvent>,
    step_id: WorkflowStepId,
) {
    emit(event_tx, DeterministicOrchestratorEvent::Halted { step_id });
}

/// Arguments for [`emit_step_progress`].
pub struct StepProgressArgs {
    /// Step that produced the progress event.
    pub step_id: WorkflowStepId,
    /// Normalized signal recorded for the progress update.
    pub signal: NormalizedSignal,
    /// Name of the agent that produced this signal, if known.
    pub agent_name: Option<String>,
}

/// Emits a `StepProgressed` event for the given step and signal.
pub fn emit_step_progress(ports: &RuntimePorts, args: StepProgressArgs) {
    emit(
        &ports.event_tx,
        DeterministicOrchestratorEvent::StepProgressed {
            step_id: args.step_id,
            signal: args.signal,
            agent_name: args.agent_name,
        },
    );
}
