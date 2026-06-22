use augur_domain::domain::dag_validation::validate_execution_plan;
use augur_domain::domain::task_types::{
    ExecutionPlan, ExecutionStepId, ExecutionStepSpec, RawStepId,
};

fn step_id(s: &str) -> ExecutionStepId {
    ExecutionStepId::new(RawStepId::new(s)).expect("valid step id")
}

fn simple_step(id: &str) -> ExecutionStepSpec {
    ExecutionStepSpec {
        step_id: step_id(id),
        intent_name: id.to_owned().into(),
        depends_on: vec![],
        required_artifacts: vec![],
        produces: vec![],
    }
}

#[test]
fn valid_single_step_plan_succeeds() {
    let plan = ExecutionPlan::new(vec![simple_step("step-a")], None);
    assert!(validate_execution_plan(plan).is_ok());
}

#[test]
fn valid_two_step_sequential_plan_succeeds() {
    let step_b = ExecutionStepSpec {
        step_id: step_id("step-b"),
        intent_name: "b".to_owned().into(),
        depends_on: vec![step_id("step-a")],
        required_artifacts: vec![],
        produces: vec![],
    };
    let plan = ExecutionPlan::new(vec![simple_step("step-a"), step_b], None);
    assert!(validate_execution_plan(plan).is_ok());
}

#[test]
fn duplicate_step_id_returns_error() {
    let plan = ExecutionPlan::new(vec![simple_step("step-a"), simple_step("step-a")], None);
    assert!(validate_execution_plan(plan).is_err());
}

#[test]
fn undefined_dependency_returns_error() {
    let step = ExecutionStepSpec {
        step_id: step_id("step-a"),
        intent_name: "a".to_owned().into(),
        depends_on: vec![step_id("step-missing")],
        required_artifacts: vec![],
        produces: vec![],
    };
    let plan = ExecutionPlan::new(vec![step], None);
    assert!(validate_execution_plan(plan).is_err());
}
