use super::*;

/// Advance to the next unexecuted parallel-group member or transition the group to the next step on completion.
///
/// Looks up the owning group step, finds the first member without an execution
/// record, and either starts it or resolves the group's pass transition when
/// all members are done.
pub(super) async fn advance_parallel_group_or_next_member(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    member_step: &WorkflowStep,
) {
    let Some(group_step_id) = parallel_group_step_id_for_member(state, &member_step.id) else {
        emit_halted(&ports.event_tx, member_step.id.clone());
        state.run_state.current_step_id = None;
        return;
    };
    let Some(group_step) = super::workflow_step(state, &group_step_id).cloned() else {
        emit_halted(&ports.event_tx, member_step.id.clone());
        state.run_state.current_step_id = None;
        return;
    };

    let next_member = group_step.execution.members.iter().find(|m| {
        !state
            .run_state
            .prior_steps
            .iter()
            .any(|r| r.step_id == m.id)
    });

    if let Some(next_member) = next_member {
        state.run_state.current_step_id = Some(next_member.id.clone());
        Box::pin(super::start_current_step(state, ports)).await;
        return;
    }

    match resolve_pass_transition(&group_step, &NormalizedSignal::Advance) {
        PassTransitionResolution::AdvanceTo(next_step_id) => {
            super::transition_to_declared_step_target(
                state,
                ports,
                DeclaredStepTransition {
                    from_step_id: group_step_id,
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
            emit_halted(&ports.event_tx, group_step_id);
            state.run_state.current_step_id = None;
        }
    }
}

fn ensure_group_placeholder_record(
    state: &mut DeterministicOrchestratorRunState,
    group_step_id: &WorkflowStepId,
) {
    let already_present = state
        .run_state
        .prior_steps
        .iter()
        .any(|record| record.step_id == *group_step_id);
    if !already_present {
        state.run_state.prior_steps.push(
            StepExecutionRecord::builder()
                .step_id(group_step_id.clone())
                .worker_signal(NormalizedSignal::Advance)
                .updated_artifacts(vec![])
                .build(),
        );
    }
}

fn find_group_record_mut<'a>(
    state: &'a mut DeterministicOrchestratorRunState,
    group_step_id: &WorkflowStepId,
) -> Option<&'a mut StepExecutionRecord> {
    state
        .run_state
        .prior_steps
        .iter_mut()
        .rev()
        .find(|record| record.step_id == *group_step_id)
}

/// Append the evaluated step's outcome to the owning parallel group's member-results list.
///
/// Creates a placeholder group record if one does not yet exist, then pushes a
/// `GroupMemberResult` containing the step ID, agent name, and transition signal.
pub(super) fn record_parallel_group_member_result(
    state: &mut DeterministicOrchestratorRunState,
    evaluated: &EvaluatedStep,
) {
    let Some(group_step_id) = parallel_group_step_id_for_member(state, &evaluated.step.id) else {
        return;
    };
    let Some(agent_name) = member_result_agent_name(&evaluated.step) else {
        tracing::warn!(
            step_id = %evaluated.step.id,
            group_step_id = %group_step_id,
            "parallel group member result missing dispatch agent; skipping tracking"
        );
        return;
    };
    ensure_group_placeholder_record(state, &group_step_id);

    let Some(group_record) = find_group_record_mut(state, &group_step_id) else {
        tracing::warn!(
            step_id = %evaluated.step.id,
            group_step_id = %group_step_id,
            "parallel group record missing after placeholder creation; skipping member result tracking"
        );
        return;
    };

    group_record.remediation_record.member_results.push(
        GroupMemberResult::builder()
            .step_id(evaluated.step.id.clone())
            .agent_name(agent_name)
            .signal(evaluated.transition_signal.clone())
            .maybe_failure_decision(
                evaluated
                    .execution
                    .remediation_record
                    .failure_decision
                    .clone(),
            )
            .build(),
    );
}

/// Return the `WorkflowStepId` of the parallel group that owns the given member step, if any.
///
/// Scans the step index for a `ParallelGroup` step whose `members` list
/// contains `member_step_id`, returning `None` when no such group exists.
pub(super) fn parallel_group_step_id_for_member(
    state: &DeterministicOrchestratorRunState,
    member_step_id: &WorkflowStepId,
) -> Option<WorkflowStepId> {
    state
        .progress
        .step_index
        .first_executable_by_declared_step_id
        .keys()
        .find_map(|step_id| {
            let step = super::workflow_step(state, step_id)?;
            let is_parallel_group = step.kind == WorkflowStepKind::ParallelGroup;
            let contains_member = step
                .execution
                .members
                .iter()
                .any(|member| &member.id == member_step_id);

            if is_parallel_group && contains_member {
                Some(step.id.clone())
            } else {
                None
            }
        })
}

fn member_result_agent_name(step: &WorkflowStep) -> Option<AgentName> {
    if step.kind.requires_evaluator().0 {
        step.dispatch.evaluator_agent.clone()
    } else {
        step.dispatch.worker_agent.clone()
    }
}

/// Return the most recent non-empty slice of `GroupMemberResult` from the prior-steps history.
///
/// Searches `prior_steps` in reverse order and returns the first record that
/// has at least one member result, or `None` if no such record exists.
pub(super) fn latest_parallel_group_member_results(
    state: &DeterministicOrchestratorRunState,
) -> Option<&[GroupMemberResult]> {
    state.run_state.prior_steps.iter().rev().find_map(|record| {
        let member_results = record.remediation_record.member_results.as_slice();
        if member_results.is_empty() {
            None
        } else {
            Some(member_results)
        }
    })
}

/// Build a `WorkflowDispatchRequest` that retries a single failed parallel-group member.
///
/// Looks up the member step in the step index, clones the worker dispatch
/// request, then overrides the worker agent with the one recorded in
/// `member_result` and clears any evaluator agent.
pub(super) fn build_member_retry_dispatch_request(
    state: &DeterministicOrchestratorRunState,
    member_result: &GroupMemberResult,
) -> Option<WorkflowDispatchRequest> {
    let member_step = super::workflow_step(state, &member_result.step_id)?;
    let mut request =
        build_worker_dispatch_request(member_step, state.progress.feature_context.clone());
    request.dispatch.worker_agent = Some(member_result.agent_name.clone());
    request.dispatch.evaluator_agent = None;
    Some(request)
}
