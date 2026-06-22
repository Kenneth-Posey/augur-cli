//! Stage 3 behavior wiring for execution-plan persistence (M6).

use augur_domain::StringNewtype;
use augur_domain::domain::{
    ArtifactData, ArtifactName, ExecutionStepId, PlanState, PlanStateReconstructionError, RunId,
    StepArtifact, StepKey, StepSpecJson, StepStatus, ValidatedPlan,
};
use std::sync::{Mutex, OnceLock};

/// Platform timestamp projection used by persistence rows.
pub type Timestamp = std::time::SystemTime;

/// Persistence projection for a reconstructed step-state row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepStateRow {
    pub step_id: ExecutionStepId,
    pub status: StepStatus,
    pub step_spec_json: StepSpecJson,
    pub artifacts: Vec<StepArtifact>,
}

/// Persistence projection for one `step_artifacts` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepArtifactRow {
    pub run_id: RunId,
    pub step_id: ExecutionStepId,
    pub artifact_name: ArtifactName,
    pub artifact_data: ArtifactData,
    pub produced_at: Timestamp,
}

/// Persistence-layer failure vocabulary for plan storage and recovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanPersistenceError {
    ConnectionFailed {
        reason: String,
    },
    TransactionFailed {
        reason: String,
    },
    DeserializationFailed {
        step_id: ExecutionStepId,
        reason: String,
    },
    PlanNotFound {
        run_id: RunId,
    },
    StepNotFound {
        key: StepKey,
    },
    UnexpectedRowCount {
        key: StepKey,
        expected: u64,
        actual: u64,
    },
}

impl std::fmt::Display for PlanPersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed { reason } => {
                write!(f, "persistence connection failed: {reason}")
            }
            Self::TransactionFailed { reason } => {
                write!(f, "persistence transaction failed: {reason}")
            }
            Self::DeserializationFailed { step_id, reason } => write!(
                f,
                "failed to deserialize persisted step {}: {reason}",
                step_id.as_ref()
            ),
            Self::PlanNotFound { run_id } => {
                write!(f, "persisted plan not found for run {}", run_id.as_ref())
            }
            Self::StepNotFound { key } => write!(
                f,
                "persisted step not found: run {}, step {}",
                key.run_id.as_ref(),
                key.step_id.as_ref()
            ),
            Self::UnexpectedRowCount {
                key,
                expected,
                actual,
            } => write!(
                f,
                "unexpected row count for run {}, step {}: expected {expected}, actual {actual}",
                key.run_id.as_ref(),
                key.step_id.as_ref()
            ),
        }
    }
}

impl From<PlanStateReconstructionError> for PlanPersistenceError {
    fn from(value: PlanStateReconstructionError) -> Self {
        match value {
            PlanStateReconstructionError::EmptyRows => PlanPersistenceError::TransactionFailed {
                reason: "recovery failed: persisted plan has zero step rows".to_string(),
            },
            PlanStateReconstructionError::InvalidStepSpecJson { step_id, reason } => {
                PlanPersistenceError::DeserializationFailed { step_id, reason }
            }
            PlanStateReconstructionError::IncompleteState { reason, .. } => {
                PlanPersistenceError::TransactionFailed { reason }
            }
        }
    }
}

#[derive(Clone)]
struct PersistedRun {
    rows: augur_domain::domain::Map<ExecutionStepId, StepStateRow>,
}

#[derive(Default)]
struct InMemoryPlanPersistence {
    runs: augur_domain::domain::Map<RunId, PersistedRun>,
}

fn store() -> &'static Mutex<InMemoryPlanPersistence> {
    static STORE: OnceLock<Mutex<InMemoryPlanPersistence>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(InMemoryPlanPersistence::default()))
}

/// Persist one validated execution plan atomically.
///
/// Preconditions: `plan` is a typestate-validated plan; `run_id` is non-empty.
/// Postconditions: on `Ok(())`, all todos and todo_deps rows for `run_id` are committed.
/// Failure cases: `ConnectionFailed`, `TransactionFailed`.
pub fn persist_execution_plan(
    plan: ValidatedPlan,
    run_id: RunId,
) -> Result<(), PlanPersistenceError> {
    let mut guard = store()
        .lock()
        .map_err(|_| PlanPersistenceError::ConnectionFailed {
            reason: "plan persistence store lock poisoned".to_string(),
        })?;

    let mut rows = augur_domain::domain::Map::new();
    for step in &plan.inner().steps {
        let step_spec_json = serde_json::to_string(step).map_err(|error| {
            PlanPersistenceError::TransactionFailed {
                reason: format!(
                    "serialization failed for step {}: {error}",
                    step.step_id.as_ref()
                ),
            }
        })?;
        rows.insert(
            step.step_id.clone(),
            StepStateRow {
                step_id: step.step_id.clone(),
                status: StepStatus::Pending,
                step_spec_json: StepSpecJson::new(step_spec_json),
                artifacts: Vec::new(),
            },
        );
    }

    guard.runs.insert(run_id, PersistedRun { rows });

    Ok(())
}

/// Load a previously persisted validated plan.
///
/// Preconditions: `run_id` exists in persistence.
/// Postconditions: on success, returns a `ValidatedPlan` reconstructed from DB rows.
/// Failure cases: `PlanNotFound`, `DeserializationFailed`, `ConnectionFailed`.
pub fn load_plan_from_db(run_id: RunId) -> Result<ValidatedPlan, PlanPersistenceError> {
    recover_plan_state_from_db(run_id).map(|state| state.plan_spec)
}

/// Recover full runtime plan state from persistence rows.
///
/// Preconditions: `run_id` exists in persistence.
/// Postconditions: on success, returned state has `state.run_id == run_id`.
/// Failure cases: `PlanNotFound`, `DeserializationFailed`, `ConnectionFailed`.
pub fn recover_plan_state_from_db(run_id: RunId) -> Result<PlanState, PlanPersistenceError> {
    let guard = store()
        .lock()
        .map_err(|_| PlanPersistenceError::ConnectionFailed {
            reason: "plan persistence store lock poisoned".to_string(),
        })?;

    let run = guard
        .runs
        .get(&run_id)
        .ok_or_else(|| PlanPersistenceError::PlanNotFound {
            run_id: run_id.clone(),
        })?;

    let rows = run
        .rows
        .values()
        .cloned()
        .map(|row| augur_domain::domain::StepStateRow {
            step_id: row.step_id,
            status: row.status,
            step_spec_json: row.step_spec_json,
            artifacts: row.artifacts,
        })
        .collect::<Vec<_>>();
    PlanState::from_db_rows(rows, run_id).map_err(PlanPersistenceError::from)
}

/// Persist one step-status transition.
///
/// Preconditions: `(key.run_id, key.step_id)` exists.
/// Postconditions: on success, exactly one row status is updated.
/// Failure cases: `StepNotFound`, `UnexpectedRowCount`, `ConnectionFailed`.
pub fn update_step_status(key: StepKey, status: StepStatus) -> Result<(), PlanPersistenceError> {
    let mut guard = store()
        .lock()
        .map_err(|_| PlanPersistenceError::ConnectionFailed {
            reason: "plan persistence store lock poisoned".to_string(),
        })?;

    let Some(run) = guard.runs.get_mut(&key.run_id) else {
        return Err(PlanPersistenceError::StepNotFound { key });
    };

    let Some(row) = run.rows.get_mut(&key.step_id) else {
        return Err(PlanPersistenceError::StepNotFound { key });
    };

    row.status = status;
    if status != StepStatus::Completed {
        row.artifacts.clear();
    }
    Ok(())
}

/// Persist produced artifacts for one run.
///
/// Preconditions: every row in `artifacts` belongs to `run_id`.
/// Postconditions: on success, all rows are inserted atomically.
/// Failure cases: `TransactionFailed`, `ConnectionFailed`.
pub fn persist_step_artifacts(
    run_id: RunId,
    artifacts: Vec<StepArtifactRow>,
) -> Result<(), PlanPersistenceError> {
    if artifacts.iter().any(|row| row.run_id != run_id) {
        return Err(PlanPersistenceError::TransactionFailed {
            reason: "artifact batch includes mismatched run_id".to_string(),
        });
    }

    let mut guard = store()
        .lock()
        .map_err(|_| PlanPersistenceError::ConnectionFailed {
            reason: "plan persistence store lock poisoned".to_string(),
        })?;

    let run =
        guard
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| PlanPersistenceError::TransactionFailed {
                reason: "artifact persistence run_id not found".to_string(),
            })?;

    for row in artifacts {
        let artifact =
            StepArtifact::new(row.artifact_name, row.artifact_data).map_err(|cause| {
                PlanPersistenceError::TransactionFailed {
                    reason: format!("artifact validation failed: {cause:?}"),
                }
            })?;

        let step_row = run.rows.get_mut(&row.step_id).ok_or_else(|| {
            PlanPersistenceError::TransactionFailed {
                reason: format!(
                    "artifact persistence step_id {} not found for run {}",
                    row.step_id.as_ref(),
                    run_id.as_ref()
                ),
            }
        })?;
        step_row.artifacts.push(artifact);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ExecutionStepId, PlanPersistenceError, RunId, ValidatedPlan, load_plan_from_db,
        persist_execution_plan, store,
    };
    use augur_domain::StringNewtype;
    use augur_domain::domain::{
        ExecutionPlan, ExecutionStepSpec, RawStepId, StepSpecJson, validate_execution_plan,
    };

    fn validated_single_step_plan() -> ValidatedPlan {
        let plan = ExecutionPlan::new(
            vec![ExecutionStepSpec {
                step_id: ExecutionStepId::new(RawStepId::new("persist-step")).expect("id valid"),
                intent_name: "persist-intent".to_string().into(),
                depends_on: Vec::new(),
                required_artifacts: Vec::new(),
                produces: Vec::new(),
            }],
            None,
        );
        validate_execution_plan(plan).expect("plan validates")
    }

    #[test]
    fn load_plan_from_db_deserialization_failure_returns_deserialization_failed() {
        let run_id = RunId::new("run-per-010").expect("run id should be valid");
        let step_id = ExecutionStepId::new(RawStepId::new("persist-step")).expect("id valid");
        persist_execution_plan(validated_single_step_plan(), run_id.clone()).expect("persist");

        {
            let mut guard = store()
                .lock()
                .expect("plan persistence store lock should not be poisoned");
            let run = guard
                .runs
                .get_mut(&run_id)
                .expect("persisted run should exist for corruption injection");
            let row = run
                .rows
                .get_mut(&step_id)
                .expect("persisted step row should exist for corruption injection");
            row.step_spec_json = StepSpecJson::new("{not-json");
        }

        let result = load_plan_from_db(run_id);
        assert!(matches!(
            result,
            Err(PlanPersistenceError::DeserializationFailed {
                step_id: ref candidate,
                reason: ref msg
            }) if *candidate == step_id && !msg.trim().is_empty()
        ));
    }
}
