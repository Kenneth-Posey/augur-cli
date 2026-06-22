use augur_domain::domain::dag_validation::validate_execution_plan;
use augur_domain::domain::plan_state::PlanState;
use augur_domain::domain::scheduler::{
    ReplyDecision, apply_step_completion, ready_steps, reply_decision,
};
use augur_domain::domain::task_types::{
    ExecutionPlan, ExecutionStepId, ExecutionStepSpec, RawStepId, RunId, StepStatus,
};

fn step_id(s: &str) -> ExecutionStepId {
    ExecutionStepId::new(RawStepId::new(s)).unwrap()
}

fn run_id(s: &str) -> RunId {
    RunId::new(s).unwrap()
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

fn single_step_plan() -> PlanState {
    let plan = ExecutionPlan::new(vec![simple_step("a")], None);
    let validated = validate_execution_plan(plan).unwrap();
    PlanState::new(validated, run_id("run-1"))
}

#[test]
fn ready_steps_returns_pending_steps_with_no_deps() {
    let state = single_step_plan();
    let ready = ready_steps(state);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0], step_id("a"));
}

#[test]
fn reply_decision_is_not_yet_when_steps_pending() {
    let state = single_step_plan();
    assert_eq!(reply_decision(state), ReplyDecision::NotYet);
}

#[test]
fn reply_decision_is_ready_after_all_steps_complete() {
    let mut state = single_step_plan();
    state.step_states.get_mut(&step_id("a")).unwrap().status = StepStatus::Completed;
    assert_eq!(reply_decision(state), ReplyDecision::ReadyToReply);
}

#[test]
fn apply_step_completion_marks_step_completed() {
    let mut state = single_step_plan();
    state.step_states.get_mut(&step_id("a")).unwrap().status = StepStatus::Running;
    apply_step_completion(step_id("a"), vec![], &mut state);
    assert_eq!(
        state.step_states[&step_id("a")].status,
        StepStatus::Completed
    );
}
