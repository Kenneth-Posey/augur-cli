//! Stage 3.2 signature surfaces for orchestrator ingestion and scheduling (M7).

use crate::persistence::plan_persistence::{
    PlanPersistenceError, StepArtifactRow, persist_execution_plan, persist_step_artifacts,
    update_step_status,
};
use augur_domain::domain::{
    ExecutionPlan, ExecutionPlanError, Map, OrchestratorEvent, PlanState, RunId, StepArtifact,
    StepKey, StepStatus, apply_step_completion, build_wait_or_reply_event, ready_steps,
    validate_execution_plan,
};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

/// Terminal outcome for one execution step callback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    Completed { artifacts: Vec<StepArtifact> },
    Failed { reason: String },
}

/// Actor-layer failure vocabulary for ingestion/scheduling operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrchestratorError {
    InvalidPlan { cause: ExecutionPlanError },
    PersistenceFailed { cause: PlanPersistenceError },
    StepNotRunning { key: StepKey },
    PlanNotFound { run_id: RunId },
    InvariantViolation { message: String },
}

impl std::fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPlan { cause } => write!(f, "invalid execution plan: {cause}"),
            Self::PersistenceFailed { cause } => write!(f, "plan persistence failed: {cause}"),
            Self::StepNotRunning { key } => write!(
                f,
                "step is not running: run {}, step {}",
                key.run_id.as_ref(),
                key.step_id.as_ref()
            ),
            Self::PlanNotFound { run_id } => {
                write!(f, "active plan not found for run {}", run_id.as_ref())
            }
            Self::InvariantViolation { message } => {
                write!(f, "orchestrator invariant violation: {message}")
            }
        }
    }
}

/// Opaque orchestration context shared by ingestion and timeout handlers.
#[derive(Clone, Debug, Default)]
pub struct OrchestratorContext {
    pub active_plans: Arc<Mutex<Map<RunId, PlanState>>>,
}

impl OrchestratorContext {
    /// Create a fresh orchestrator context with an empty active-plan registry.
    pub fn new() -> Self {
        Self {
            active_plans: Arc::new(Mutex::new(Map::new())),
        }
    }
}

fn derive_run_id(
    validated: &augur_domain::domain::ValidatedPlan,
) -> Result<RunId, OrchestratorError> {
    let encoded = serde_json::to_string(validated.inner()).map_err(|error| {
        OrchestratorError::InvariantViolation {
            message: format!("failed to encode validated plan for run-id derivation: {error}"),
        }
    })?;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    encoded.hash(&mut hasher);
    let value = hasher.finish();
    RunId::new(format!("run_{value:016x}"))
        .map_err(|cause| OrchestratorError::InvalidPlan { cause })
}

/// Validate, persist, and register one execution plan run.
///
/// Preconditions: `plan` may be unvalidated; validation occurs internally.
/// Postconditions: on success, returns a fresh `RunId` and registers `PlanState::new`.
/// Failure cases: `InvalidPlan`, `PersistenceFailed`.
pub fn submit_execution_plan(
    plan: ExecutionPlan,
    ctx: OrchestratorContext,
) -> Result<RunId, OrchestratorError> {
    let validated =
        validate_execution_plan(plan).map_err(|cause| OrchestratorError::InvalidPlan { cause })?;
    let run_id = derive_run_id(&validated)?;

    {
        let guard = ctx
            .active_plans
            .lock()
            .map_err(|_| OrchestratorError::InvariantViolation {
                message: "active plan map lock poisoned".to_string(),
            })?;
        if guard.contains_key(&run_id) {
            return Err(OrchestratorError::InvalidPlan {
                cause: ExecutionPlanError::PlanAlreadyExists {
                    run_id: run_id.clone(),
                },
            });
        }
    }

    persist_execution_plan(validated.clone(), run_id.clone())
        .map_err(|cause| OrchestratorError::PersistenceFailed { cause })?;

    let mut guard = ctx
        .active_plans
        .lock()
        .map_err(|_| OrchestratorError::InvariantViolation {
            message: "active plan map lock poisoned".to_string(),
        })?;
    guard.insert(run_id.clone(), PlanState::new(validated, run_id.clone()));

    Ok(run_id)
}

/// Execute one scheduler tick for a run and return the conversation event.
///
/// Preconditions: `run_id` is present in `ctx.active_plans`.
/// Postconditions: ready steps are transitioned to `Running`, then one event is returned.
/// Failure cases: `PlanNotFound`, `PersistenceFailed`, `InvariantViolation`.
pub fn drive_scheduler_tick(
    run_id: RunId,
    ctx: OrchestratorContext,
) -> Result<OrchestratorEvent, OrchestratorError> {
    let mut guard = ctx
        .active_plans
        .lock()
        .map_err(|_| OrchestratorError::InvariantViolation {
            message: "active plan map lock poisoned".to_string(),
        })?;

    let state = guard
        .get_mut(&run_id)
        .ok_or_else(|| OrchestratorError::PlanNotFound {
            run_id: run_id.clone(),
        })?;

    let ready_snapshot = ready_steps(state.clone());
    for step_id in ready_snapshot {
        if let Some(step_state) = state.step_states.get_mut(&step_id) {
            step_state.status = StepStatus::Running;
            step_state.error_reason = None;
        }

        update_step_status(StepKey::new(run_id.clone(), step_id), StepStatus::Running)
            .map_err(|cause| OrchestratorError::PersistenceFailed { cause })?;
    }

    Ok(build_wait_or_reply_event(state.clone(), run_id))
}

/// Handle a terminal callback for one running step.
///
/// Preconditions: targeted step exists and is `Running`.
/// Postconditions: persists terminal status and triggers one follow-up scheduler tick.
/// Failure cases: `StepNotRunning`, `PlanNotFound`, `PersistenceFailed`.
pub fn handle_step_terminal(
    key: StepKey,
    outcome: StepOutcome,
    ctx: OrchestratorContext,
) -> Result<(), OrchestratorError> {
    {
        let mut guard =
            ctx.active_plans
                .lock()
                .map_err(|_| OrchestratorError::InvariantViolation {
                    message: "active plan map lock poisoned".to_string(),
                })?;

        let state = state_for_running_step(&mut guard, &key)?;
        apply_terminal_outcome(state, &key, outcome)?;
    }

    let _ = drive_scheduler_tick(key.run_id.clone(), ctx)?;
    Ok(())
}

fn state_for_running_step<'a>(
    guard: &'a mut Map<RunId, PlanState>,
    key: &StepKey,
) -> Result<&'a mut PlanState, OrchestratorError> {
    let state = guard
        .get_mut(&key.run_id)
        .ok_or_else(|| OrchestratorError::PlanNotFound {
            run_id: key.run_id.clone(),
        })?;
    ensure_step_running(state, key)?;
    Ok(state)
}

fn ensure_step_running(state: &PlanState, key: &StepKey) -> Result<(), OrchestratorError> {
    let step_state =
        state
            .step_states
            .get(&key.step_id)
            .ok_or_else(|| OrchestratorError::PlanNotFound {
                run_id: key.run_id.clone(),
            })?;
    if step_state.status != StepStatus::Running {
        return Err(OrchestratorError::StepNotRunning { key: key.clone() });
    }
    Ok(())
}

fn apply_terminal_outcome(
    state: &mut PlanState,
    key: &StepKey,
    outcome: StepOutcome,
) -> Result<(), OrchestratorError> {
    match outcome {
        StepOutcome::Completed { artifacts } => handle_completed_outcome(state, key, artifacts),
        StepOutcome::Failed { reason } => handle_failed_outcome(state, key, reason),
    }
}

fn handle_completed_outcome(
    state: &mut PlanState,
    key: &StepKey,
    artifacts: Vec<StepArtifact>,
) -> Result<(), OrchestratorError> {
    let row_artifacts = to_artifact_rows(key, &artifacts);
    apply_step_completion(key.step_id.clone(), artifacts, state);
    persist_step_artifacts(key.run_id.clone(), row_artifacts)
        .map_err(|cause| OrchestratorError::PersistenceFailed { cause })?;
    update_step_status(key.clone(), StepStatus::Completed)
        .map_err(|cause| OrchestratorError::PersistenceFailed { cause })
}

fn handle_failed_outcome(
    state: &mut PlanState,
    key: &StepKey,
    reason: String,
) -> Result<(), OrchestratorError> {
    if let Some(step) = state.step_states.get_mut(&key.step_id) {
        step.status = StepStatus::Failed;
        step.error_reason = Some(reason);
        step.artifacts.clear();
    }
    update_step_status(key.clone(), StepStatus::Failed)
        .map_err(|cause| OrchestratorError::PersistenceFailed { cause })
}

fn to_artifact_rows(key: &StepKey, artifacts: &[StepArtifact]) -> Vec<StepArtifactRow> {
    artifacts
        .iter()
        .map(|artifact| StepArtifactRow {
            run_id: key.run_id.clone(),
            step_id: key.step_id.clone(),
            artifact_name: artifact.name().as_ref().to_string().into(),
            artifact_data: artifact.data().as_ref().to_string().into(),
            produced_at: std::time::SystemTime::now(),
        })
        .collect()
}
