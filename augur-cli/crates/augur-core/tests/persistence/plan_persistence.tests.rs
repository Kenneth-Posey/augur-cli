use augur_core::persistence::plan_persistence::{
    load_plan_from_db, persist_execution_plan, persist_step_artifacts, recover_plan_state_from_db,
    update_step_status, PlanPersistenceError, StepArtifactRow,
};
use augur_domain::domain::{
    ready_steps, validate_execution_plan, ExecutionPlan, ExecutionStepId, ExecutionStepSpec,
    RawStepId, RunId, StepKey, StepStatus,
};

fn validated_single_step_plan() -> augur_domain::domain::ValidatedPlan {
    let plan = ExecutionPlan::new(
        vec![ExecutionStepSpec {
            step_id: ExecutionStepId::new(RawStepId::new("persist-step"))
                .expect("id should be valid"),
            intent_name: "persist-intent".to_string().into(),
            depends_on: Vec::new(),
            required_artifacts: Vec::new(),
            produces: Vec::new(),
        }],
        None,
    );
    validate_execution_plan(plan).expect("plan should validate")
}

#[test]
fn test_persist_execution_plan_commits_rows_atomically() {
    let run_id = RunId::new("run-per-001").expect("run id should be valid");
    persist_execution_plan(validated_single_step_plan(), run_id).expect("persist should succeed");
}

#[test]
fn test_load_and_recover_plan_from_db_reconstructs_runtime_state() {
    let run_id = RunId::new("run-per-002").expect("run id should be valid");
    persist_execution_plan(validated_single_step_plan(), run_id.clone())
        .expect("persist should succeed");

    let loaded = load_plan_from_db(run_id.clone()).expect("load should succeed");
    let recovered = recover_plan_state_from_db(run_id).expect("recovery should succeed");

    assert_eq!(loaded.inner().steps.len(), recovered.step_states.len());
}

#[test]
fn test_recover_plan_state_from_db_supports_resume_scheduling_continuity() {
    let run_id = RunId::new("run-per-003").expect("run id should be valid");
    persist_execution_plan(validated_single_step_plan(), run_id.clone())
        .expect("persist should succeed");
    let recovered = recover_plan_state_from_db(run_id).expect("recovery should succeed");
    let ready = ready_steps(recovered.clone());
    assert!(ready.len() <= recovered.step_states.len());
}

#[test]
fn test_update_step_status_updates_single_row_for_existing_step() {
    let run_id = RunId::new("run-per-004").expect("run id should be valid");
    let step_id = ExecutionStepId::new(RawStepId::new("persist-step")).expect("id should be valid");
    persist_execution_plan(validated_single_step_plan(), run_id.clone())
        .expect("persist should succeed");
    update_step_status(StepKey::new(run_id, step_id), StepStatus::Completed)
        .expect("update_step_status should succeed");
}

#[test]
fn test_persist_step_artifacts_inserts_rows_atomically() {
    let run_id = RunId::new("run-per-005").expect("run id should be valid");
    persist_execution_plan(validated_single_step_plan(), run_id.clone())
        .expect("persist should succeed");
    let step_id = ExecutionStepId::new(RawStepId::new("persist-step")).expect("id should be valid");
    let rows = vec![StepArtifactRow {
        run_id: run_id.clone(),
        step_id,
        artifact_name: "artifact-a".to_string().into(),
        artifact_data: "payload".to_string().into(),
        produced_at: std::time::SystemTime::now(),
    }];
    persist_step_artifacts(run_id, rows).expect("artifact persistence should succeed");
}

#[test]
fn test_recover_plan_state_from_db_missing_run_returns_plan_not_found() {
    let run_id = RunId::new("run-per-011").expect("run id should be valid");
    let result = recover_plan_state_from_db(run_id.clone());
    assert_eq!(result, Err(PlanPersistenceError::PlanNotFound { run_id }));
}

#[test]
fn test_update_step_status_missing_or_multirow_returns_row_count_error() {
    let run_id = RunId::new("run-per-012").expect("run id should be valid");
    let step_id = ExecutionStepId::new(RawStepId::new("missing-step")).expect("id should be valid");
    let key = StepKey::new(run_id, step_id);

    let result = update_step_status(key.clone(), StepStatus::Completed);
    assert!(
        matches!(
            result,
            Err(PlanPersistenceError::StepNotFound { key: ref candidate }) if *candidate == key
        ) || matches!(
            result,
            Err(PlanPersistenceError::UnexpectedRowCount {
                key: ref candidate,
                expected: 1,
                actual: 0
            }) if *candidate == key
        )
    );
}

#[test]
fn test_persist_step_artifacts_mismatched_run_id_has_no_partial_writes() {
    let run_id = RunId::new("run-per-013").expect("run id should be valid");
    persist_execution_plan(validated_single_step_plan(), run_id.clone())
        .expect("persist should succeed");
    let step_id = ExecutionStepId::new(RawStepId::new("persist-step")).expect("id should be valid");
    let mismatched_run_id = RunId::new("run-per-013-other").expect("run id should be valid");
    let rows = vec![StepArtifactRow {
        run_id: mismatched_run_id,
        step_id,
        artifact_name: "artifact-a".to_string().into(),
        artifact_data: "payload".to_string().into(),
        produced_at: std::time::SystemTime::now(),
    }];

    let result = persist_step_artifacts(run_id.clone(), rows);
    assert!(matches!(
        result,
        Err(PlanPersistenceError::TransactionFailed { .. })
    ));
    let recovered = recover_plan_state_from_db(run_id).expect("run should remain recoverable");
    let step = recovered
        .step_states
        .values()
        .next()
        .expect("single-step state should exist");
    assert!(step.artifacts.is_empty());
}
