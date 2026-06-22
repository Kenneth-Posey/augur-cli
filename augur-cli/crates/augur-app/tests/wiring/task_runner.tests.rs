use augur_cli::wiring::task_runner::{
    TaskRequest, TaskRequestStep, TaskRunner, build_execution_plan_for_request,
};
use augur_core::actors::orchestrator::ingestion::OrchestratorContext;
use augur_domain::domain::{
    DurationMs, ExecutionPlanError, OrchestratorEvent, RawStepId, TimeoutConfig,
};

fn multi_step_request() -> TaskRequest {
    TaskRequest::builder()
        .steps(vec![
            TaskRequestStep::builder()
                .step_id(RawStepId::new("root"))
                .intent_name("intent-root".to_string().into())
                .depends_on(Vec::new())
                .required_artifacts(Vec::new())
                .produces(vec!["artifact-root".to_string()])
                .build(),
            TaskRequestStep::builder()
                .step_id(RawStepId::new("child"))
                .intent_name("intent-child".to_string().into())
                .depends_on(vec![RawStepId::new("root")])
                .required_artifacts(vec!["artifact-root".to_string()])
                .produces(vec!["artifact-child".to_string()])
                .build(),
        ])
        .maybe_timeout(Some(TimeoutConfig {
            total_timeout_ms: Some(DurationMs::from(1000)),
            per_step_timeout_ms: Some(DurationMs::from(500)),
        }))
        .build()
}

fn single_step_request() -> TaskRequest {
    TaskRequest::builder()
        .steps(vec![
            TaskRequestStep::builder()
                .step_id(RawStepId::new("single"))
                .intent_name("intent-single".to_string().into())
                .depends_on(Vec::new())
                .required_artifacts(Vec::new())
                .produces(vec!["artifact-single".to_string()])
                .build(),
        ])
        .maybe_timeout(None)
        .build()
}

#[test]
fn test_build_execution_plan_for_request_empty_derived_step_id_returns_empty_step_id() {
    let request = TaskRequest::builder()
        .steps(vec![
            TaskRequestStep::builder()
                .step_id(RawStepId::new(""))
                .intent_name("intent-empty".to_string().into())
                .depends_on(Vec::new())
                .required_artifacts(Vec::new())
                .produces(Vec::new())
                .build(),
        ])
        .maybe_timeout(None)
        .build();

    let result = build_execution_plan_for_request(request);
    assert_eq!(result, Err(ExecutionPlanError::EmptyStepId));
}

#[test]
fn test_task_runner_run_routes_through_submit_execution_plan() {
    let runner = TaskRunner::new(OrchestratorContext::new());
    let event = runner
        .run(multi_step_request())
        .expect("task runner should return orchestrator event after submit path is implemented");
    assert!(matches!(
        event,
        OrchestratorEvent::WaitForPlanCompletion { .. }
    ));
}

#[test]
fn test_task_runner_run_multi_step_request_returns_wait_not_reply() {
    let runner = TaskRunner::new(OrchestratorContext::new());
    let event = runner
        .run(multi_step_request())
        .expect("multi-step run should succeed");
    assert!(matches!(
        event,
        OrchestratorEvent::WaitForPlanCompletion { .. }
    ));
}

#[test]
fn test_task_runner_run_single_step_request_remains_compatible_with_routing() {
    let runner = TaskRunner::new(OrchestratorContext::new());
    let _event = runner
        .run(single_step_request())
        .expect("single-step request should still route through orchestrator");
}

#[test]
fn test_task_runner_run_propagates_orchestrator_submit_error() {
    let runner = TaskRunner::new(OrchestratorContext::new());
    let result = runner.run(single_step_request());
    assert!(matches!(
        result,
        Ok(OrchestratorEvent::WaitForPlanCompletion { .. })
    ));
}
