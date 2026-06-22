use super::*;

/// Handle an agent execution failure for the current orchestration step.
///
/// Emits a `Hold` progress signal, then routes to the appropriate worker or
/// evaluator failure handler based on the `dispatch_kind` of the failure.
pub(super) async fn handle_agent_execution_failure(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    failure: AgentExecutionFailure,
) {
    let Some(context) = agent_execution_failure_context(state, &failure) else {
        return;
    };
    emit_step_progress(
        ports,
        StepProgressArgs {
            step_id: failure.step_id.clone(),
            signal: NormalizedSignal::Hold,
            agent_name: context.agent_name,
        },
    );
    route_agent_execution_failure(
        state,
        ports,
        AgentExecutionFailureRoute {
            failure,
            step: context.step,
        },
    )
    .await;
}

struct AgentExecutionFailureContext {
    step: WorkflowStep,
    agent_name: Option<String>,
}

fn agent_execution_failure_context(
    state: &DeterministicOrchestratorRunState,
    failure: &AgentExecutionFailure,
) -> Option<AgentExecutionFailureContext> {
    let current_step_id = state.run_state.current_step_id.as_ref()?;
    super::same_step_id(current_step_id, &failure.step_id)?;
    let step = super::current_step(state).cloned()?;
    Some(AgentExecutionFailureContext {
        agent_name: failure_agent_name(&step, &failure.dispatch_kind),
        step,
    })
}

fn failure_agent_name(step: &WorkflowStep, kind: &DispatchRequestKind) -> Option<String> {
    match kind {
        DispatchRequestKind::Worker => step.dispatch.worker_agent.as_ref().map(|a| a.to_string()),
        DispatchRequestKind::Evaluator => step
            .dispatch
            .evaluator_agent
            .as_ref()
            .map(|a| a.to_string()),
    }
}

struct AgentExecutionFailureRoute {
    failure: AgentExecutionFailure,
    step: WorkflowStep,
}

async fn route_agent_execution_failure(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    route: AgentExecutionFailureRoute,
) {
    match route.failure.dispatch_kind {
        DispatchRequestKind::Worker => {
            handle_worker_dispatch_failure(state, ports, route.step).await
        }
        DispatchRequestKind::Evaluator => {
            handle_evaluator_dispatch_failure(
                state,
                ports,
                EvaluatorDispatchFailure {
                    step: route.step,
                    step_id: route.failure.step_id,
                },
            )
            .await;
        }
    }
}

/// Determine whether a `NeedsRevision` signal should trigger remediation or a cycle-guard hold.
///
/// Returns `Remediate` the first time a step fails and `HoldCycleGuard` when a
/// remediation has already been attempted, preventing infinite retry loops.
pub(super) fn needs_revision_routing(
    step_id: &WorkflowStepId,
    records: &[StepExecutionRecord],
) -> NeedsRevisionRouting {
    let already_attempted = records
        .iter()
        .rev()
        .find(|r| &r.step_id == step_id)
        .map(|r| r.remediation_record.remediation_attempted.0)
        .unwrap_or(false);
    if already_attempted {
        NeedsRevisionRouting::HoldCycleGuard
    } else {
        NeedsRevisionRouting::Remediate
    }
}

fn remediation_patch_agent(step: &WorkflowStep) -> Option<AgentName> {
    step.transition.on_needs_revision.quick_patch_agent.clone()
}

fn remediation_failure_notes(state: &DeterministicOrchestratorRunState) -> Option<&OutputText> {
    state
        .run_state
        .pending_failure
        .as_ref()
        .and_then(|failure| failure.failure_notes.as_ref())
}

fn build_original_worker_retry_request(
    state: &DeterministicOrchestratorRunState,
    step: &WorkflowStep,
) -> WorkflowDispatchRequest {
    build_worker_dispatch_request(step, state.progress.feature_context.clone())
}

/// Dispatch a quick-patch agent and, on success, retry any failed parallel-group members or the original worker.
///
/// Returns `NormalizedSignal::Advance` when all retried work passes, or
/// `NormalizedSignal::Hold` when no patch agent is configured, the patch
/// fails, or a member retry fails.
pub(super) async fn dispatch_remediation(
    state: &DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step: &WorkflowStep,
) -> NormalizedSignal {
    let Some(patch_agent) = remediation_patch_agent(step) else {
        tracing::debug!(
            step_id = %step.id,
            "dispatch_remediation: no quick_patch_agent configured - returning Hold",
        );
        return NormalizedSignal::Hold;
    };

    if !dispatch_quick_patch_phase(
        ports,
        QuickPatchPhaseRequest {
            step,
            patch_agent: &patch_agent,
            failure_notes: remediation_failure_notes(state),
        },
    )
    .await
    {
        tracing::debug!(
            step_id = %step.id,
            "dispatch_remediation: quick-patch phase did not pass - returning Hold",
        );
        return NormalizedSignal::Hold;
    }

    if let Some(member_results) = super::latest_parallel_group_member_results(state) {
        return retry_failed_parallel_members(
            state,
            ports,
            RetryFailedMembersArgs {
                step,
                member_results,
            },
        )
        .await;
    }

    tracing::warn!(
        step_id = %step.id,
        "dispatch_remediation: no prior parallel-group member results found; retrying original worker"
    );
    let retry_request = build_original_worker_retry_request(state, step);
    super::dispatch_patch_agent_and_await(ports, &step.id, retry_request).await
}

struct QuickPatchPhaseRequest<'a> {
    step: &'a WorkflowStep,
    patch_agent: &'a AgentName,
    failure_notes: Option<&'a OutputText>,
}

async fn dispatch_quick_patch_phase(
    ports: &RuntimePorts,
    request: QuickPatchPhaseRequest<'_>,
) -> bool {
    let patch_request =
        build_patch_dispatch_request(request.patch_agent, request.step, request.failure_notes);
    let patch_signal =
        super::dispatch_patch_agent_and_await(ports, &request.step.id, patch_request).await;
    patch_signal == NormalizedSignal::Advance
}

struct RetryFailedMembersArgs<'a> {
    step: &'a WorkflowStep,
    member_results: &'a [GroupMemberResult],
}

async fn retry_failed_parallel_members(
    state: &DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    args: RetryFailedMembersArgs<'_>,
) -> NormalizedSignal {
    for member_result in args
        .member_results
        .iter()
        .filter(|member_result| member_result.signal != NormalizedSignal::Advance)
    {
        let Some(retry_request) = super::build_member_retry_dispatch_request(state, member_result)
        else {
            tracing::warn!(
                step_id = %args.step.id,
                member_step_id = %member_result.step_id,
                "dispatch_remediation: failing checker missing from step index - returning Hold"
            );
            return NormalizedSignal::Hold;
        };
        let retry_step_id = retry_request.step_id.clone();
        let retry_signal =
            super::dispatch_patch_agent_and_await(ports, &retry_step_id, retry_request).await;
        if retry_signal != NormalizedSignal::Advance {
            return NormalizedSignal::Hold;
        }
    }
    NormalizedSignal::Advance
}

/// Apply needs-revision routing: attempt remediation on the first occurrence, fall back to failure policy on repeat.
///
/// Marks the attempt as tried, dispatches remediation, and if successful
/// transitions to the next step. Otherwise delegates to `automatically_apply_failure_policy`.
pub(super) async fn apply_needs_revision_routing(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step: &WorkflowStep,
) {
    if needs_revision_routing(&step.id, &state.run_state.prior_steps)
        == NeedsRevisionRouting::Remediate
    {
        handle_remediation_routing(state, ports, step).await;
        return;
    }
    automatically_apply_failure_policy(state, ports, step).await;
}

async fn handle_remediation_routing(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step: &WorkflowStep,
) {
    mark_remediation_attempted(state, &step.id);
    let final_signal = dispatch_remediation(state, ports, step).await;
    if final_signal != NormalizedSignal::Advance {
        automatically_apply_failure_policy(state, ports, step).await;
        return;
    }
    state.run_state.pending_failure = None;
    route_remediation_pass_transition(state, ports, step).await;
}

fn mark_remediation_attempted(
    state: &mut DeterministicOrchestratorRunState,
    step_id: &WorkflowStepId,
) {
    if let Some(last) = state
        .run_state
        .prior_steps
        .last_mut()
        .filter(|last| last.step_id == *step_id)
    {
        last.remediation_record.remediation_attempted = true.into();
    }
}

async fn route_remediation_pass_transition(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step: &WorkflowStep,
) {
    match resolve_pass_transition(step, &NormalizedSignal::Advance) {
        PassTransitionResolution::AdvanceTo(next_step_id) => {
            super::transition_to_declared_step_target(
                state,
                ports,
                DeclaredStepTransition {
                    from_step_id: step.id.clone(),
                    target_step_id: next_step_id,
                },
            )
            .await;
        }
        PassTransitionResolution::Complete => {
            state.run_state.current_step_id = None;
            emit(&ports.event_tx, DeterministicOrchestratorEvent::Completed);
        }
        PassTransitionResolution::StayOnCurrentStep => {
            automatically_apply_failure_policy(state, ports, step).await;
        }
    }
}

/// Select and apply the configured failure policy for the current step.
///
/// Resolves the `FailureDecision` via `selected_failure_decision` and then
/// calls `apply_failure_policy` to execute the chosen transition.
pub(super) async fn automatically_apply_failure_policy(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step: &WorkflowStep,
) {
    let pending_failure = state.run_state.pending_failure.clone();
    let decision = selected_failure_decision(state, step, pending_failure.as_ref());

    apply_failure_policy(
        state,
        ports,
        AppliedDecision {
            step_id: step.id.clone(),
            decision,
        },
    )
    .await;
}

/// Choose the `FailureDecision` to apply given the current run state and any pending failure context.
///
/// Returns `None` when the step uses a declared automatic transition and no
/// explicit decision is required, or `Some(FailureDecision::Halt)` as a safe
/// fallback when decision logic fails.
pub(super) fn selected_failure_decision(
    state: &DeterministicOrchestratorRunState,
    step: &WorkflowStep,
    pending_failure: Option<&PendingFailureContext>,
) -> Option<FailureDecision> {
    if step
        .transition
        .on_fail
        .action
        .uses_declared_automatic_transition()
        .0
    {
        return None;
    }
    let Some(pending_failure) = pending_failure else {
        tracing::warn!(step_id = %step.id, "failure policy selection missing pending failure context");
        return Some(FailureDecision::Halt);
    };
    choose_failure_decision(
        state.failure_policy.as_ref(),
        FailureDecisionInput {
            step,
            pending_failure,
            step_index: &state.progress.step_index,
            executed_steps: &state.progress.executed_steps,
            run_state: &state.run_state,
        },
    )
    .unwrap_or_else(|error| {
        tracing::warn!(step_id = %step.id, error = %error, "failure policy selection failed");
        Some(FailureDecision::Halt)
    })
}

/// Execute a `FailureDecision` and update orchestrator state accordingly.
///
/// Resolves the failure transition, clears pending worker and failure state,
/// then routes to rerun, backtrack, delegate-fix, continue, or halt as
/// indicated by the resolved `FailureTransitionResolution`.
pub(super) async fn apply_failure_policy(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    applied: AppliedDecision,
) {
    let Some(step) = super::workflow_step(state, &applied.step_id).cloned() else {
        emit_halted(&ports.event_tx, applied.step_id);
        state.run_state.current_step_id = None;
        state.pending_worker = None;
        state.run_state.pending_failure = None;
        return;
    };

    annotate_last_failure_decision(state, &applied);

    let resolution = resolve_failure_transition(
        &step,
        applied.decision.as_ref(),
        FailureTransitionContext {
            step_index: &state.progress.step_index,
            executed_steps: &state.progress.executed_steps,
            run_state: &state.run_state,
        },
    );

    state.pending_worker = None;
    state.run_state.pending_failure = None;

    route_failure_resolution(
        state,
        ports,
        FailureResolutionContext {
            applied,
            step,
            resolution,
        },
    )
    .await;
}

/// Route a resolved failure to a delegate-fix handler or a step transition, halting if neither applies.
///
/// Checks for a `DelegateFix` resolution first, then attempts a step
/// transition. If both return without action the orchestrator halts.
pub(super) async fn route_failure_resolution(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    context: FailureResolutionContext,
) {
    if let Some(delegate_args) = delegate_fix_args(&context) {
        super::handle_delegate_fix(state, ports, delegate_args).await;
        return;
    }
    if route_failure_step_transition(state, ports, &context).await {
        return;
    }
    state.run_state.current_step_id = None;
    emit_halted(&ports.event_tx, context.applied.step_id);
}

/// Extract `DelegateFixArgs` when the failure resolution is a `DelegateFix` variant.
///
/// Returns `None` for all other `FailureTransitionResolution` variants so the
/// caller can branch without exhaustive matching.
pub(super) fn delegate_fix_args(context: &FailureResolutionContext) -> Option<DelegateFixArgs> {
    if let FailureTransitionResolution::DelegateFix {
        patch_agent,
        return_to_reviewer,
        attempt,
        failure_notes,
    } = &context.resolution
    {
        return Some(
            DelegateFixArgs::builder()
                .step_id(context.applied.step_id.clone())
                .patch_agent(patch_agent.clone())
                .return_to_reviewer(return_to_reviewer.clone())
                .attempt(*attempt)
                .maybe_failure_notes(failure_notes.clone())
                .build(),
        );
    }
    None
}

/// Apply the step-level failure transition (rerun, backtrack, or continue) and return whether a transition occurred.
///
/// Returns `true` if a rerun, backtrack, or continue-to-next-step transition
/// was applied; returns `false` for `Halt` or `DelegateFix` resolutions that
/// require separate handling.
pub(super) async fn route_failure_step_transition(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    context: &FailureResolutionContext,
) -> bool {
    match &context.resolution {
        FailureTransitionResolution::RerunCurrentStep => {
            handle_rerun_current_step(state, ports, context.step.id.clone()).await;
            true
        }
        FailureTransitionResolution::BacktrackTo(target_step_id) => {
            handle_backtrack_to(
                state,
                ports,
                BacktrackTarget {
                    from_step_id: context.applied.step_id.clone(),
                    target_step_id: target_step_id.clone(),
                },
            )
            .await;
            true
        }
        FailureTransitionResolution::ContinueToNextStep(next_step_id) => {
            super::transition_to_declared_step_target(
                state,
                ports,
                DeclaredStepTransition {
                    from_step_id: context.applied.step_id.clone(),
                    target_step_id: next_step_id.clone(),
                },
            )
            .await;
            true
        }
        FailureTransitionResolution::Halt | FailureTransitionResolution::DelegateFix { .. } => {
            false
        }
    }
}

async fn handle_rerun_current_step(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step_id: WorkflowStepId,
) {
    state.run_state.current_step_id = Some(step_id.clone());
    emit(
        &ports.event_tx,
        DeterministicOrchestratorEvent::RerunScheduled { step_id },
    );
    Box::pin(super::start_current_step(state, ports)).await;
}

async fn handle_backtrack_to(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    target: BacktrackTarget,
) {
    state.run_state.current_step_id = Some(target.target_step_id.clone());
    emit(
        &ports.event_tx,
        DeterministicOrchestratorEvent::Backtracked {
            from_step_id: target.from_step_id,
            to_step_id: target.target_step_id,
        },
    );
    Box::pin(super::start_current_step(state, ports)).await;
}
