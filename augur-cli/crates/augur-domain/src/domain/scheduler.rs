//! Scheduling helpers for plan-step readiness and reply decisions.

use crate::domain::newtypes::IsPredicate;
use crate::domain::{ExecutionStepId, PlanState, StepArtifact, StepStatus};

/// High-level reply decision derived from current plan state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyDecision {
    /// Continue waiting; work is still running or launchable.
    NotYet,
    /// No running/ready work remains and no step has failed.
    ReadyToReply,
    /// At least one step failed and no running/ready work remains.
    ErrorAbortReply,
}

/// Return all pending steps whose dependencies are fully completed.
pub fn ready_steps(state: PlanState) -> Vec<ExecutionStepId> {
    ready_steps_ref(&state)
}

/// Return whether a specific step may be launched now.
pub fn can_launch_step(step_id: ExecutionStepId, state: PlanState) -> IsPredicate {
    IsPredicate::from(ready_steps_ref(&state).into_iter().any(|id| id == step_id))
}

/// Apply a successful step completion transition when the step is running.
pub fn apply_step_completion(
    step_id: ExecutionStepId,
    artifacts: Vec<StepArtifact>,
    state: &mut PlanState,
) {
    if let Some(step_state) = state.step_states.get_mut(&step_id) {
        if step_state.status == StepStatus::Completed {
            return;
        }

        if step_state.status == StepStatus::Running {
            step_state.status = StepStatus::Completed;
            step_state.artifacts = artifacts;
            step_state.error_reason = None;
        }
    }
}

/// Decide whether orchestration should wait, reply, or abort.
pub fn reply_decision(state: PlanState) -> ReplyDecision {
    reply_decision_ref(&state)
}

/// Aggregate final artifacts from completed steps using deterministic winner rules.
pub fn aggregate_step_artifacts(state: PlanState) -> Vec<StepArtifact> {
    aggregate_step_artifacts_ref(&state)
}

fn ready_steps_ref(state: &PlanState) -> Vec<ExecutionStepId> {
    let spec_by_id = step_spec_map(state);

    state
        .step_states
        .iter()
        .filter_map(|(step_id, step_state)| {
            if step_state.status != StepStatus::Pending {
                return None;
            }

            let step_spec = spec_by_id.get(step_id)?;
            let deps_completed = step_spec.depends_on.iter().all(|dep| {
                state
                    .step_states
                    .get(dep)
                    .map(|dep_state| dep_state.status == StepStatus::Completed)
                    .unwrap_or(false)
            });

            if deps_completed {
                Some(step_id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn reply_decision_ref(state: &PlanState) -> ReplyDecision {
    let running = state
        .step_states
        .values()
        .any(|step_state| step_state.status == StepStatus::Running);
    if running {
        return ReplyDecision::NotYet;
    }

    if !ready_steps_ref(state).is_empty() {
        return ReplyDecision::NotYet;
    }

    let any_failed = state
        .step_states
        .values()
        .any(|step_state| step_state.status == StepStatus::Failed);

    if any_failed {
        ReplyDecision::ErrorAbortReply
    } else {
        ReplyDecision::ReadyToReply
    }
}

fn aggregate_step_artifacts_ref(state: &PlanState) -> Vec<StepArtifact> {
    let mut winners: std::collections::BTreeMap<String, (ExecutionStepId, StepArtifact)> =
        std::collections::BTreeMap::new();

    for (step_id, step_state) in &state.step_states {
        if step_state.status != StepStatus::Completed {
            continue;
        }

        for artifact in &step_state.artifacts {
            if should_replace_winner(step_id, artifact, &winners) {
                winners.insert(
                    artifact.name().as_ref().to_string(),
                    (step_id.clone(), artifact.clone()),
                );
            }
        }
    }

    let mut records: Vec<(ExecutionStepId, StepArtifact)> = winners.into_values().collect();
    records.sort_by(|(left_step, left_artifact), (right_step, right_artifact)| {
        left_step.cmp(right_step).then(
            left_artifact
                .name()
                .as_ref()
                .cmp(right_artifact.name().as_ref()),
        )
    });

    records.into_iter().map(|(_, artifact)| artifact).collect()
}

fn should_replace_winner(
    step_id: &ExecutionStepId,
    artifact: &StepArtifact,
    winners: &std::collections::BTreeMap<String, (ExecutionStepId, StepArtifact)>,
) -> bool {
    let Some((winner_step_id, _)) = winners.get(artifact.name().as_ref()) else {
        return true;
    };
    step_id > winner_step_id
}

fn step_spec_map(
    state: &PlanState,
) -> std::collections::BTreeMap<ExecutionStepId, &crate::domain::ExecutionStepSpec> {
    state
        .plan_spec
        .inner()
        .steps
        .iter()
        .map(|step| (step.step_id.clone(), step))
        .collect()
}
