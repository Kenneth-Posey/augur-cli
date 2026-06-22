use augur_domain::domain::dag_validation::validate_execution_plan;
use augur_domain::domain::plan_state::PlanState;
use augur_domain::domain::scheduler::{
    ReplyDecision, apply_step_completion, ready_steps, reply_decision,
};
use augur_domain::domain::task_types::RawStepId;
use augur_domain::domain::{ExecutionPlan, ExecutionStepId, ExecutionStepSpec, RunId, StepStatus};

fn step_id(s: &str) -> ExecutionStepId {
    ExecutionStepId::new(RawStepId::new(s)).unwrap()
}

fn make_state(step_ids: &[&str]) -> PlanState {
    let steps = step_ids
        .iter()
        .map(|s| ExecutionStepSpec {
            step_id: step_id(s),
            intent_name: s.to_string().into(),
            depends_on: vec![],
            required_artifacts: vec![],
            produces: vec![],
        })
        .collect();
    let plan = validate_execution_plan(ExecutionPlan::new(steps, None)).unwrap();
    PlanState::new(plan, RunId::new("run-1").unwrap())
}

#[test]
fn ready_steps_returns_all_pending_with_no_deps() {
    let state = make_state(&["a", "b"]);
    let ready = ready_steps(state);
    assert_eq!(ready.len(), 2);
}

#[test]
fn reply_decision_is_not_yet_when_steps_pending() {
    let state = make_state(&["a"]);
    assert_eq!(reply_decision(state), ReplyDecision::NotYet);
}

#[test]
fn apply_step_completion_marks_running_step_completed() {
    let mut state = make_state(&["a"]);
    let id = step_id("a");
    state.step_states.get_mut(&id).unwrap().status = StepStatus::Running;
    apply_step_completion(id.clone(), vec![], &mut state);
    assert_eq!(state.step_states[&id].status, StepStatus::Completed);
}

#[test]
fn apply_step_completion_is_noop_for_already_completed() {
    let mut state = make_state(&["a"]);
    let id = step_id("a");
    state.step_states.get_mut(&id).unwrap().status = StepStatus::Completed;
    apply_step_completion(id.clone(), vec![], &mut state);
    assert_eq!(state.step_states[&id].status, StepStatus::Completed);
}
