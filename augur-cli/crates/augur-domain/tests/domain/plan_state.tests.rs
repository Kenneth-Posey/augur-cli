use augur_domain::domain::plan_state::PlanStateReconstructionError;

#[test]
fn plan_state_types_exist() {
    // Placeholder: plan_state module tests
    // Module exports StepStateRow, PlanStateReconstructionError for plan persistence
    // Real tests will verify state reconstruction and error handling
}

#[test]
fn plan_state_reconstruction_error() {
    let err = PlanStateReconstructionError::EmptyRows;
    let display = format!("{}", err);
    assert!(!display.is_empty());
}
