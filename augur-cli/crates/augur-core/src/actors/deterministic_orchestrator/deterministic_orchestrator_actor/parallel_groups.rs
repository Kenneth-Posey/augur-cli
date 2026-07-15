use super::*;

/// Dispatch all parallel group members simultaneously or process a completed member.
///
/// On first entry (no pending members set), dispatches all members in parallel.
/// On subsequent entries (member completion), checks if all members are done
/// and resolves the group transition.
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

    // If we haven't started dispatching parallel members yet, dispatch all at once
    if state.pending_parallel_members.is_empty() {
        dispatch_all_parallel_members(state, ports, &group_step).await;
        return;
    }

    // Remove this member from the pending set
    state.pending_parallel_members.retain(|id| id != &member_step.id);

    // If there are still pending members, wait for them
    if !state.pending_parallel_members.is_empty() {
        return;
    }

    // All members completed - resolve the group transition
    let group_member_results = latest_parallel_group_member_results(state);
    let all_passed = group_member_results
        .map(|results| results.iter().all(|r| r.signal == NormalizedSignal::Advance))
        .unwrap_or(false);

    let transition_signal = if all_passed {
        NormalizedSignal::Advance
    } else {
        NormalizedSignal::Hold
    };

    state.active_parallel_group_id = None;

    match resolve_pass_transition(&group_step, &transition_signal) {
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

/// Dispatches all executable members of a parallel group simultaneously.
async fn dispatch_all_parallel_members(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    group_step: &WorkflowStep,
) {
    state.active_parallel_group_id = Some(group_step.id.clone());

    // Collect all executable members
    let executable_members: Vec<&WorkflowStep> = group_step
        .execution
        .members
        .iter()
        .filter(|m| m.kind.is_executable().0)
        .collect();

    // Track their IDs
    state.pending_parallel_members = executable_members
        .iter()
        .map(|m| m.id.clone())
        .collect();

    // Create a placeholder group record
    ensure_group_placeholder_record(state, &group_step.id);

    // Dispatch all members simultaneously
    for member in &executable_members {
        state.artifact_store.pre_create_output_dirs(member);
        dispatch_request(
            ports,
            state.artifact_store.clone(),
            build_worker_dispatch_request(member, state.progress.feature_context.clone()),
            &state.agent_instructions,
        )
        .await;
    }
}

/// Returns true when the given step_id is part of a pending parallel group.
pub(super) fn is_pending_parallel_member(
    state: &DeterministicOrchestratorRunState,
    step_id: &WorkflowStepId,
) -> bool {
    state.pending_parallel_members.iter().any(|id| id == step_id)
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
