//! Persistable plan-execution state and reconstruction helpers.

use crate::domain::string_newtypes::StepSpecJson;
use crate::domain::{
    ExecutionPlan, ExecutionStepId, ExecutionStepSpec, Map, RunId, StepArtifact, StepStatus,
    ValidatedPlan,
};

/// Persisted row shape used to reconstruct a [`PlanState`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepStateRow {
    /// Step identifier.
    pub step_id: ExecutionStepId,
    /// Persisted runtime status.
    pub status: StepStatus,
    /// Serialized [`ExecutionStepSpec`] JSON.
    pub step_spec_json: StepSpecJson,
    /// Persisted artifacts (terminal completed rows only).
    pub artifacts: Vec<StepArtifact>,
}

/// Errors produced while rebuilding [`PlanState`] from persisted rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanStateReconstructionError {
    /// No rows were supplied.
    EmptyRows,
    /// A row had malformed `step_spec_json`.
    InvalidStepSpecJson {
        /// Step id for the malformed row.
        step_id: ExecutionStepId,
        /// Parse failure details.
        reason: String,
    },
    /// Rows were internally inconsistent or incomplete.
    IncompleteState {
        /// Correlated run id.
        run_id: RunId,
        /// Human-readable reason.
        reason: String,
    },
}

impl std::fmt::Display for PlanStateReconstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRows => write!(f, "cannot reconstruct plan state from empty row set"),
            Self::InvalidStepSpecJson { step_id, reason } => write!(
                f,
                "invalid step-spec json for step {}: {reason}",
                step_id.as_ref()
            ),
            Self::IncompleteState { run_id, reason } => {
                write!(
                    f,
                    "incomplete persisted state for run {}: {reason}",
                    run_id.as_ref()
                )
            }
        }
    }
}

/// Runtime state for a single execution step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepState {
    /// Step identifier.
    pub step_id: ExecutionStepId,
    /// Current runtime status.
    pub status: StepStatus,
    /// Produced artifacts (completed steps only).
    pub artifacts: Vec<StepArtifact>,
    /// Optional failure reason when status is `Failed`.
    pub error_reason: Option<String>,
}

/// In-memory state for one plan run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanState {
    /// Correlated run id.
    pub run_id: RunId,
    /// Runtime status rows keyed by step id.
    pub step_states: Map<ExecutionStepId, StepState>,
    /// Validated immutable plan specification.
    pub plan_spec: ValidatedPlan,
}

impl PlanState {
    /// Build a new pending-state plan from a validated spec and run id.
    pub fn new(plan: ValidatedPlan, run_id: RunId) -> Self {
        let mut step_states = Map::new();
        for step in &plan.inner().steps {
            step_states.insert(
                step.step_id.clone(),
                StepState {
                    step_id: step.step_id.clone(),
                    status: StepStatus::Pending,
                    artifacts: Vec::new(),
                    error_reason: None,
                },
            );
        }

        Self {
            run_id,
            step_states,
            plan_spec: plan,
        }
    }

    /// Reconstruct plan state from persisted rows for a run id.
    pub fn from_db_rows(
        rows: Vec<StepStateRow>,
        run_id: RunId,
    ) -> Result<Self, PlanStateReconstructionError> {
        if rows.is_empty() {
            return Err(PlanStateReconstructionError::EmptyRows);
        }

        let rebuilt = rebuild_state_maps(rows, &run_id)?;
        let plan_spec = plan_spec_from_specs(rebuilt.specs);
        validate_state_cardinality(&rebuilt.states, &plan_spec, &run_id)?;

        Ok(Self {
            run_id,
            step_states: rebuilt.states,
            plan_spec,
        })
    }

    fn rebuild_step_state(
        row: &StepStateRow,
        run_id: RunId,
    ) -> Result<StepState, PlanStateReconstructionError> {
        match row.status {
            StepStatus::Pending | StepStatus::Running => non_terminal_step_state(row, run_id),
            StepStatus::Completed => Ok(completed_step_state(row)),
            StepStatus::Failed => failed_step_state(row, run_id),
        }
    }
}

struct RebuiltStateMaps {
    specs: Map<ExecutionStepId, ExecutionStepSpec>,
    states: Map<ExecutionStepId, StepState>,
}

fn rebuild_state_maps(
    rows: Vec<StepStateRow>,
    run_id: &RunId,
) -> Result<RebuiltStateMaps, PlanStateReconstructionError> {
    let mut specs: Map<ExecutionStepId, ExecutionStepSpec> = Map::new();
    let mut states: Map<ExecutionStepId, StepState> = Map::new();
    let mut ctx = RowRebuildCtx {
        run_id,
        specs: &mut specs,
        states: &mut states,
    };

    for row in rows {
        rebuild_row_into_maps(row, &mut ctx)?;
    }

    Ok(RebuiltStateMaps { specs, states })
}

struct RowRebuildCtx<'a> {
    run_id: &'a RunId,
    specs: &'a mut Map<ExecutionStepId, ExecutionStepSpec>,
    states: &'a mut Map<ExecutionStepId, StepState>,
}

fn rebuild_row_into_maps(
    row: StepStateRow,
    ctx: &mut RowRebuildCtx<'_>,
) -> Result<(), PlanStateReconstructionError> {
    let row_step_id = row.step_id.clone();
    let spec = parse_step_spec(&row)?;
    validate_matching_step_id(&spec, &row_step_id, ctx.run_id)?;
    insert_unique_spec(ctx.specs, spec, ctx.run_id)?;

    let state = PlanState::rebuild_step_state(&row, ctx.run_id.clone())?;
    ctx.states.insert(row_step_id, state);
    Ok(())
}

fn plan_spec_from_specs(specs: Map<ExecutionStepId, ExecutionStepSpec>) -> ValidatedPlan {
    let steps: Vec<ExecutionStepSpec> = specs.into_values().collect();
    ValidatedPlan::from_validated(ExecutionPlan::new(steps, None))
}

fn validate_state_cardinality(
    states: &Map<ExecutionStepId, StepState>,
    plan_spec: &ValidatedPlan,
    run_id: &RunId,
) -> Result<(), PlanStateReconstructionError> {
    if states.len() == plan_spec.inner().steps.len() {
        return Ok(());
    }
    Err(PlanStateReconstructionError::IncompleteState {
        run_id: run_id.clone(),
        reason: "step-state cardinality mismatch".to_string(),
    })
}

fn parse_step_spec(row: &StepStateRow) -> Result<ExecutionStepSpec, PlanStateReconstructionError> {
    serde_json::from_str(&row.step_spec_json).map_err(|err| {
        PlanStateReconstructionError::InvalidStepSpecJson {
            step_id: row.step_id.clone(),
            reason: err.to_string(),
        }
    })
}

fn validate_matching_step_id(
    spec: &ExecutionStepSpec,
    row_step_id: &ExecutionStepId,
    run_id: &RunId,
) -> Result<(), PlanStateReconstructionError> {
    if spec.step_id == *row_step_id {
        return Ok(());
    }
    Err(PlanStateReconstructionError::IncompleteState {
        run_id: run_id.clone(),
        reason: "step id mismatch".to_string(),
    })
}

fn insert_unique_spec(
    specs: &mut Map<ExecutionStepId, ExecutionStepSpec>,
    spec: ExecutionStepSpec,
    run_id: &RunId,
) -> Result<(), PlanStateReconstructionError> {
    if specs.insert(spec.step_id.clone(), spec).is_none() {
        return Ok(());
    }
    Err(PlanStateReconstructionError::IncompleteState {
        run_id: run_id.clone(),
        reason: "duplicate step row".to_string(),
    })
}

fn non_terminal_step_state(
    row: &StepStateRow,
    run_id: RunId,
) -> Result<StepState, PlanStateReconstructionError> {
    ensure_no_artifacts(row, run_id, "non-terminal step has artifacts")?;
    Ok(StepState {
        step_id: row.step_id.clone(),
        status: row.status,
        artifacts: Vec::new(),
        error_reason: None,
    })
}

fn completed_step_state(row: &StepStateRow) -> StepState {
    StepState {
        step_id: row.step_id.clone(),
        status: StepStatus::Completed,
        artifacts: row.artifacts.clone(),
        error_reason: None,
    }
}

fn failed_step_state(
    row: &StepStateRow,
    run_id: RunId,
) -> Result<StepState, PlanStateReconstructionError> {
    ensure_no_artifacts(row, run_id, "failed step has artifacts")?;
    Ok(StepState {
        step_id: row.step_id.clone(),
        status: StepStatus::Failed,
        artifacts: Vec::new(),
        error_reason: Some("recovered_failed_state".to_string()),
    })
}

fn ensure_no_artifacts(
    row: &StepStateRow,
    run_id: RunId,
    reason: &str,
) -> Result<(), PlanStateReconstructionError> {
    if row.artifacts.is_empty() {
        return Ok(());
    }
    Err(PlanStateReconstructionError::IncompleteState {
        run_id,
        reason: reason.to_string(),
    })
}
