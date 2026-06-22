//! Stage 3.2 signature surfaces for timeout enforcement (M8).

use crate::actors::orchestrator::ingestion::{
    OrchestratorContext, OrchestratorError, StepOutcome, drive_scheduler_tick, handle_step_terminal,
};
use crate::persistence::plan_persistence::update_step_status;
use augur_domain::domain::{PlanState, RunId, StepKey, StepStatus, build_wait_or_reply_event};

/// Handle one per-step timeout callback.
///
/// Preconditions: target step is `Running` and exceeded configured per-step timeout.
/// Postconditions: step is persisted as `Failed`, then scheduling is re-driven.
/// Failure cases: `StepNotRunning`, `PlanNotFound`, `PersistenceFailed`.
pub fn step_timeout_handler(
    key: StepKey,
    ctx: OrchestratorContext,
) -> Result<(), OrchestratorError> {
    handle_step_terminal(
        key,
        StepOutcome::Failed {
            reason: "step_timeout after <N>ms".to_string(),
        },
        ctx,
    )
}

/// Handle one plan-level timeout callback.
///
/// Preconditions: `run_id` is present in `ctx.active_plans` and exceeded total timeout.
/// Postconditions: all pending/running steps transition to `Failed` and are persisted.
/// Failure cases: `PlanNotFound`, `PersistenceFailed`, `InvariantViolation`.
pub fn plan_timeout_handler(
    run_id: RunId,
    ctx: OrchestratorContext,
) -> Result<(), OrchestratorError> {
    {
        let mut guard =
            ctx.active_plans
                .lock()
                .map_err(|_| OrchestratorError::InvariantViolation {
                    message: "active plan map lock poisoned".to_string(),
                })?;

        let state = guard
            .get_mut(&run_id)
            .ok_or_else(|| OrchestratorError::PlanNotFound {
                run_id: run_id.clone(),
            })?;
        apply_plan_timeout_to_steps(&run_id, state)?;

        let _event = build_wait_or_reply_event(state.clone(), run_id.clone());
    }

    let _ = drive_scheduler_tick(run_id, ctx)?;
    Ok(())
}

fn apply_plan_timeout_to_steps(
    run_id: &RunId,
    state: &mut PlanState,
) -> Result<(), OrchestratorError> {
    for (step_id, step_state) in &mut state.step_states {
        let Some(reason) = plan_timeout_reason(step_state.status) else {
            continue;
        };
        step_state.status = StepStatus::Failed;
        step_state.error_reason = Some(reason.to_owned());
        step_state.artifacts.clear();
        update_step_status(
            StepKey::new(run_id.clone(), step_id.clone()),
            StepStatus::Failed,
        )
        .map_err(|cause| OrchestratorError::PersistenceFailed { cause })?;
    }
    Ok(())
}

fn plan_timeout_reason(status: StepStatus) -> Option<&'static str> {
    match status {
        StepStatus::Running => Some("plan_timeout"),
        StepStatus::Pending => Some("plan_canceled_due_to_timeout"),
        _ => None,
    }
}
