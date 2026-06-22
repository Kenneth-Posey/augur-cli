//! Orchestrator-facing reply event construction from scheduler decisions.

use crate::domain::{
    PlanState, ReplyDecision, RunId, StepArtifact, StepStatus, aggregate_step_artifacts,
    ready_steps, reply_decision,
};

const WAIT_REASON_TRAILING_PAREN: char = ')';
const PLAN_TIMEOUT_REASON_PREFIX: &str = "plan timeout after";
const PLAN_TIMEOUT_REASON_TOKEN: &str = "plan_timeout";
const PLAN_TIMEOUT_CANCELED_TOKEN: &str = "plan_canceled_due_to_timeout";
const PLAN_TIMEOUT_ABORT_ERROR: &str = "plan timeout after configured limit";

/// Event emitted to orchestration based on current plan completion state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrchestratorEvent {
    /// Keep waiting while plan work is still in progress.
    WaitForPlanCompletion {
        /// Correlated plan id.
        plan_id: RunId,
        /// Human-readable reason for waiting.
        reason: String,
    },
    /// Emit a final reply payload.
    ReplyToConversation {
        /// Correlated plan id.
        plan_id: RunId,
        /// Aggregated output artifacts.
        artifacts: Vec<StepArtifact>,
    },
    /// Abort reply because the plan entered a failure state.
    AbortReply {
        /// Correlated plan id.
        plan_id: RunId,
        /// Human-readable failure reason.
        error: String,
    },
}

/// Build the next orchestrator event from plan state.
pub fn build_wait_or_reply_event(state: PlanState, plan_id: RunId) -> OrchestratorEvent {
    match reply_decision(state.clone()) {
        ReplyDecision::NotYet => OrchestratorEvent::WaitForPlanCompletion {
            plan_id,
            reason: wait_reason(&state),
        },
        ReplyDecision::ReadyToReply => OrchestratorEvent::ReplyToConversation {
            plan_id,
            artifacts: aggregate_step_artifacts(state),
        },
        ReplyDecision::ErrorAbortReply => OrchestratorEvent::AbortReply {
            plan_id,
            error: abort_error(&state),
        },
    }
}

fn wait_reason(state: &PlanState) -> String {
    let running_count = state
        .step_states
        .values()
        .filter(|step_state| step_state.status == StepStatus::Running)
        .count();
    let ready_count = ready_steps(state.clone()).len();

    let mut reason = String::from("plan execution still in progress (running=");
    reason.push_str(&running_count.to_string());
    reason.push_str(", ready=");
    reason.push_str(&ready_count.to_string());
    reason.push(WAIT_REASON_TRAILING_PAREN);
    reason
}

fn abort_error(state: &PlanState) -> String {
    for (step_id, step_state) in &state.step_states {
        if step_state.status == StepStatus::Failed {
            return format_failed_step_error(step_id, step_state);
        }
    }

    "plan failed with unknown error".to_string()
}

fn format_failed_step_error(
    step_id: &crate::domain::ExecutionStepId,
    step_state: &crate::domain::StepState,
) -> String {
    let reason = failure_reason(step_state);
    if is_timeout_failure(&reason) {
        return PLAN_TIMEOUT_ABORT_ERROR.to_string();
    }
    format_step_failure(step_id, &reason)
}

fn failure_reason(step_state: &crate::domain::StepState) -> String {
    step_state
        .error_reason
        .clone()
        .unwrap_or_else(|| "unknown failure".to_string())
}

fn is_timeout_failure(reason: &str) -> bool {
    reason.starts_with(PLAN_TIMEOUT_REASON_PREFIX)
        || reason == PLAN_TIMEOUT_REASON_TOKEN
        || reason == PLAN_TIMEOUT_CANCELED_TOKEN
}

fn format_step_failure(step_id: &crate::domain::ExecutionStepId, reason: &str) -> String {
    let mut error = String::from("step ");
    error.push_str(step_id.as_ref());
    error.push_str(" failed: ");
    error.push_str(reason);
    error
}
