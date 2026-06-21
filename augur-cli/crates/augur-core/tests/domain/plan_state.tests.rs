use augur_domain::domain::dag_validation::validate_execution_plan;
use augur_domain::domain::plan_state::{PlanState, StepState};
use augur_domain::domain::task_types::{
    ExecutionPlan, ExecutionStepId, ExecutionStepSpec, RawStepId, RunId, StepStatus,
};

fn step_id(s: &str) -> ExecutionStepId {
    ExecutionStepId::new(RawStepId::new(s)).unwrap()
}

fn run_id(s: &str) -> RunId {
    RunId::new(s).unwrap()
}

#[test]
fn plan_state_starts_with_all_steps_pending() {
    let plan = ExecutionPlan::new(
        vec![ExecutionStepSpec {
            step_id: step_id("step-1"),
            intent_name: "first".to_owned().into(),
            depends_on: vec![],
            required_artifacts: vec![],
            produces: vec![],
        }],
        None,
    );
    let validated = validate_execution_plan(plan).unwrap();
    let state = PlanState::new(validated, run_id("run-abc"));
    let s = state.step_states.get(&step_id("step-1")).unwrap();
    assert_eq!(s.status, StepStatus::Pending);
}

#[test]
fn step_state_has_correct_step_id() {
    let plan = ExecutionPlan::new(
        vec![ExecutionStepSpec {
            step_id: step_id("my-step"),
            intent_name: "do-thing".to_owned().into(),
            depends_on: vec![],
            required_artifacts: vec![],
            produces: vec![],
        }],
        None,
    );
    let validated = validate_execution_plan(plan).unwrap();
    let state = PlanState::new(validated, run_id("run-x"));
    let s = state.step_states.get(&step_id("my-step")).unwrap();
    assert_eq!(s.step_id, step_id("my-step"));
}

#[test]
fn step_state_artifacts_empty_initially() {
    let plan = ExecutionPlan::new(
        vec![ExecutionStepSpec {
            step_id: step_id("art-step"),
            intent_name: "art".to_owned().into(),
            depends_on: vec![],
            required_artifacts: vec![],
            produces: vec![],
        }],
        None,
    );
    let validated = validate_execution_plan(plan).unwrap();
    let state = PlanState::new(validated, run_id("run-y"));
    let s: &StepState = state.step_states.get(&step_id("art-step")).unwrap();
    assert!(s.artifacts.is_empty());
}
