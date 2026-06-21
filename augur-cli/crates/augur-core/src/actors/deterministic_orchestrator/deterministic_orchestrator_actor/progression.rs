use super::*;

/// Dispatch the current workflow step's worker agent.
///
/// Validates that the step is executable, resolves its input artifacts, and
/// sends the worker dispatch request. Falls back to a `Hold` evaluation when
/// input resolution fails.
pub(super) async fn start_current_step(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
) {
    let Some(step) = super::current_step(state).cloned() else {
        return;
    };

    state.pending_worker = None;

    if !step.kind.is_executable().0 {
        tracing::warn!(step_id = %step.id, "attempted to dispatch a non-executable structural step");
        emit_halted(&ports.event_tx, step.id.clone());
        state.run_state.current_step_id = None;
        state.run_state.pending_failure = None;
        return;
    }

    if let Err(error) = state.artifact_store.resolve_step_inputs(&step) {
        tracing::warn!(step_id = %step.id, error = %error, "failed to resolve step inputs - applying failure policy");
        let execution = super::worker_execution_record(&step, NormalizedSignal::Hold);
        super::handle_step_evaluation(
            state,
            ports,
            EvaluatedStep {
                step,
                execution,
                transition_signal: NormalizedSignal::Hold,
                failure_origin: FailureOrigin::Step,
                artifact_updates: vec![],
            },
        )
        .await;
        return;
    }

    state.artifact_store.pre_create_output_dirs(&step);

    dispatch_request(
        ports,
        state.artifact_store.clone(),
        build_worker_dispatch_request(&step, state.progress.feature_context.clone()),
    )
    .await;
}

/// Dispatch the evaluator agent for a step whose worker has already completed.
///
/// Stores the pending worker execution record and artifact updates, then
/// sends the evaluator dispatch request.
pub(super) async fn dispatch_evaluator_for_step(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    dispatchable: DispatchableStep,
) {
    state.pending_worker = Some(PendingStepExecution {
        execution: dispatchable.pending.execution.clone(),
        artifact_updates: dispatchable.pending.artifact_updates,
    });
    dispatch_request(
        ports,
        state.artifact_store.clone(),
        build_evaluator_dispatch_request(
            &dispatchable.step,
            &dispatchable.pending.execution,
            state.progress.feature_context.clone(),
        ),
    )
    .await;
}

/// Process a worker completion signal and either dispatch an evaluator or proceed to step evaluation.
///
/// Emits step progress, then either hands off to `dispatch_evaluator_for_step`
/// when the step requires evaluation, or calls `handle_step_evaluation` directly.
pub(super) async fn handle_worker_completion(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    completion: StepOutcome,
) {
    let Some(current_step_id) = state.run_state.current_step_id.as_ref() else {
        return;
    };
    if current_step_id != &completion.step_id || state.pending_worker.is_some() {
        return;
    }

    let Some(step) = super::current_step(state).cloned() else {
        return;
    };

    emit_step_progress(
        ports,
        StepProgressArgs {
            step_id: completion.step_id.clone(),
            signal: completion.signal.clone(),
            agent_name: step.dispatch.worker_agent.as_ref().map(|a| a.to_string()),
        },
    );

    let worker_execution = super::worker_execution_record(&step, completion.signal.clone());
    if step.kind.requires_evaluator().0 {
        let dispatchable = DispatchableStep {
            step,
            pending: PendingStepExecution {
                execution: worker_execution,
                artifact_updates: completion.artifact_updates,
            },
        };
        dispatch_evaluator_for_step(state, ports, dispatchable).await;
        return;
    }

    super::handle_step_evaluation(
        state,
        ports,
        EvaluatedStep {
            step,
            execution: worker_execution,
            transition_signal: completion.signal,
            failure_origin: FailureOrigin::Step,
            artifact_updates: completion.artifact_updates,
        },
    )
    .await;
}

/// Process an evaluator completion signal and finalize step evaluation.
///
/// Validates the completion matches the current step and pending worker,
/// merges artifact updates, computes the transition signal, and calls
/// `handle_step_evaluation`.
pub(super) async fn handle_evaluator_completion(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    completion: StepOutcome,
) {
    let Some(context) = evaluator_completion_context(state, &completion) else {
        return;
    };
    emit_step_progress(
        ports,
        build_evaluator_progress_args(&completion, &context.step),
    );
    let execution = super::evaluator_execution_record(
        &context.worker_execution.execution,
        completion.signal.clone(),
        completion.evaluator_output.clone(),
    );
    let transition_signal = evaluator_transition_signal(&execution);
    let artifact_updates = merge_artifact_updates(
        context.worker_execution.artifact_updates,
        completion.artifact_updates,
    );
    super::handle_step_evaluation(
        state,
        ports,
        EvaluatedStep {
            step: context.step,
            execution,
            transition_signal,
            failure_origin: FailureOrigin::Step,
            artifact_updates,
        },
    )
    .await;
}

struct EvaluatorCompletionContext {
    step: WorkflowStep,
    worker_execution: PendingStepExecution,
}

fn evaluator_completion_context(
    state: &DeterministicOrchestratorRunState,
    completion: &StepOutcome,
) -> Option<EvaluatorCompletionContext> {
    let step = completion_step(state, completion)?;
    let worker_execution = completion_pending_worker(state, completion)?;
    Some(EvaluatorCompletionContext {
        step,
        worker_execution,
    })
}

fn completion_step(
    state: &DeterministicOrchestratorRunState,
    completion: &StepOutcome,
) -> Option<WorkflowStep> {
    let current_step_id = state.run_state.current_step_id.as_ref()?;
    super::same_step_id(current_step_id, &completion.step_id)?;
    super::current_step(state).cloned()
}

fn completion_pending_worker(
    state: &DeterministicOrchestratorRunState,
    completion: &StepOutcome,
) -> Option<PendingStepExecution> {
    let worker_execution = state.pending_worker.clone()?;
    super::same_step_id(&worker_execution.execution.step_id, &completion.step_id)?;
    Some(worker_execution)
}

fn build_evaluator_progress_args(
    completion: &StepOutcome,
    step: &WorkflowStep,
) -> StepProgressArgs {
    StepProgressArgs {
        step_id: completion.step_id.clone(),
        signal: completion.signal.clone(),
        agent_name: step
            .dispatch
            .evaluator_agent
            .as_ref()
            .map(|a| a.to_string()),
    }
}

/// Derive the final transition signal from a completed evaluator execution record.
///
/// Returns `Advance` when both worker and evaluator agree (or the worker held
/// and the evaluator advances), `NeedsRevision` when the worker passed but the
/// evaluator requests revision, and `Hold` for all other combinations.
pub(super) fn evaluator_transition_signal(execution: &StepExecutionRecord) -> NormalizedSignal {
    let worker_passed = execution.worker_signal == NormalizedSignal::Advance;
    let worker_held = execution.worker_signal == NormalizedSignal::Hold;

    match &execution.evaluator_record.evaluator_signal {
        Some(NormalizedSignal::Advance) if worker_passed || worker_held => {
            NormalizedSignal::Advance
        }
        Some(NormalizedSignal::NeedsRevision) if worker_passed => NormalizedSignal::NeedsRevision,
        _ => NormalizedSignal::Hold,
    }
}
