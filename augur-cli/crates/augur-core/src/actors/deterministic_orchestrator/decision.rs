//! Deterministic-orchestrator failure-decision adapter.

use std::fmt;

use crate::domain::deterministic_orchestrator::{
    FailureDecision, FailureOrigin, PendingFailureContext, WorkflowRunState, WorkflowStep,
};
use crate::domain::deterministic_orchestrator_ops::{
    validate_backtrack_target, BacktrackTargetValidation, BacktrackValidationCtx,
    ExecutedStepIndex, StepIndex,
};
use augur_domain::domain::WorkflowStepId;

/// Errors produced by failure-decision selection.
#[derive(Debug)]
pub(crate) enum DecisionError {}

impl fmt::Display for DecisionError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

impl std::error::Error for DecisionError {}

/// Replaceable policy boundary for rerun/backtrack/halt decisions.
pub(crate) trait FailureDecisionPolicy: Send + Sync {
    /// Chooses an optional failure decision without mutating runtime state.
    fn choose_failure_decision(
        &self,
        input: FailureDecisionInput<'_>,
    ) -> Result<Option<FailureDecision>, DecisionError>;
}

/// Shared read-only inputs for failure-decision selection.
#[derive(Clone, Copy)]
pub(crate) struct FailureDecisionInput<'a> {
    pub step: &'a WorkflowStep,
    pub pending_failure: &'a PendingFailureContext,
    pub step_index: &'a StepIndex,
    pub executed_steps: &'a ExecutedStepIndex,
    pub run_state: &'a WorkflowRunState,
}

/// Maximum number of times a single step may be rerun for infrastructure failures
/// within one orchestrator session before the policy falls through to backtrack or halt.
///
/// This cap is enforced per-session. Cross-session enforcement requires persistent
/// attempt tracking (e.g., via orch-query state).
const MAX_STEP_RERUNS: usize = 1;

/// Default deterministic failure-decision policy.
#[derive(Clone, Debug, Default)]
pub(crate) struct DefaultFailureDecisionPolicy;

impl FailureDecisionPolicy for DefaultFailureDecisionPolicy {
    fn choose_failure_decision(
        &self,
        input: FailureDecisionInput<'_>,
    ) -> Result<Option<FailureDecision>, DecisionError> {
        if should_rerun_current_step(input) {
            return Ok(Some(FailureDecision::RerunCurrentStep));
        }

        if let Some(decision) = delegate_fix_decision(input) {
            return Ok(Some(decision));
        }

        if let Some(step_id) = select_backtrack_target(input) {
            return Ok(Some(FailureDecision::BacktrackTo { step_id }));
        }

        Ok(Some(FailureDecision::Halt))
    }
}

/// Delegates failure-decision selection to the provided replaceable policy.
pub(crate) fn choose_failure_decision(
    policy: &dyn FailureDecisionPolicy,
    input: FailureDecisionInput<'_>,
) -> Result<Option<FailureDecision>, DecisionError> {
    policy.choose_failure_decision(input)
}

/// Returns whether the current failure should be retried before backtracking.
fn should_rerun_current_step(input: FailureDecisionInput<'_>) -> bool {
    if input.pending_failure.step_id != input.step.id {
        return false;
    }

    if input.pending_failure.origin != FailureOrigin::Infrastructure {
        return false;
    }

    let attempt_count = input.executed_steps.attempt_count(&input.step.id);
    *attempt_count > 0
        && *attempt_count <= MAX_STEP_RERUNS
        && input.run_state.current_step_id.as_ref() == Some(&input.step.id)
}

/// Returns a `DelegateFix` decision when the step has a quick-patch agent configured
/// and the current failure is the first or second step-failure attempt.
///
/// The runtime records the failing execution before it asks the policy to choose a
/// resolution, so `attempt_count` includes the current failure. That means attempt 1
/// maps to `attempt_count == 1`, attempt 2 maps to `attempt_count == 2`, and any
/// later failure must fall through to backtrack or halt.
///
/// Only fires for `FailureOrigin::Step` failures. Infrastructure failures bypass this
/// path and go to `should_rerun_current_step` instead.
///
/// Returns `None` when:
/// - The failure origin is not `Step`
/// - No `quick_patch_agent` is configured on the step
/// - The current failure is already beyond the second DelegateFix attempt
fn delegate_fix_decision(input: FailureDecisionInput<'_>) -> Option<FailureDecision> {
    if input.pending_failure.origin != FailureOrigin::Step {
        return None;
    }

    let patch_agent = input.step.transition.on_fail.quick_patch_agent.clone()?;

    let attempt_count = input.executed_steps.attempt_count(&input.step.id);
    match *attempt_count {
        1 | 2 => Some(FailureDecision::DelegateFix {
            patch_agent,
            return_to_reviewer: input.step.id.clone(),
            attempt: (*attempt_count) as u8,
        }),
        _ => None,
    }
}

/// Selects a deterministic backtrack target when one is currently valid.
fn select_backtrack_target(input: FailureDecisionInput<'_>) -> Option<WorkflowStepId> {
    preferred_backtrack_target(input).or_else(|| {
        most_recent_valid_prior_step(input.step_index, input.executed_steps, input.run_state)
    })
}

/// Returns the workflow-declared backward target when it is valid for this run.
fn preferred_backtrack_target(input: FailureDecisionInput<'_>) -> Option<WorkflowStepId> {
    let target_step_id = input.step.transition.on_fail.backward_step_id.as_ref()?;
    let ctx = BacktrackValidationCtx {
        step_index: input.step_index,
        executed_steps: input.executed_steps,
        run_state: input.run_state,
    };
    let is_valid_target =
        validate_backtrack_target(&ctx, target_step_id) == BacktrackTargetValidation::Valid;

    if is_valid_target {
        Some(target_step_id.clone())
    } else {
        None
    }
}

/// Returns the most recent previously executed step that is still a valid target.
fn most_recent_valid_prior_step(
    step_index: &StepIndex,
    executed_steps: &ExecutedStepIndex,
    run_state: &WorkflowRunState,
) -> Option<WorkflowStepId> {
    let current_step_id = run_state.current_step_id.as_ref()?;
    let current_position = step_index.executable_position(current_step_id)?;
    executed_steps
        .most_recent_step_ids()
        .find(|step_id| {
            step_index
                .executable_position(step_id)
                .is_some_and(|target_position| target_position < current_position)
        })
        .cloned()
}
