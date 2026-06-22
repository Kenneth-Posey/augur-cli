#![allow(clippy::empty_docs)]
//!

use augur_domain::domain::newtypes::IsPredicate;

use augur_core::actors::supervisor::phase_gate::{evaluate_gate, StepOutcome};
use augur_domain::domain::plan_tree::{NodeStatus, PlanNode, PlanNodeId};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};

/// Verifies that `evaluate_gate` returns `passed: IsPredicate::yes()` when the outcome
/// carries `Done` status for the correct node id and no error flag.
#[test]
fn gate_passes_when_outcome_is_done_and_no_error() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let outcome = StepOutcome {
        last_node_status: Some((node.id.clone(), NodeStatus::Done)),
        has_error: IsPredicate::no(),
        error_message: None,
    };
    let result = evaluate_gate(&node, &outcome);
    assert!(result.passed);
    assert!(result.reason.is_none());
}

/// Verifies that `evaluate_gate` returns `passed: IsPredicate::no()` when the node update
/// carries `Failed` status for the correct id, propagating the failure message.
#[test]
fn gate_fails_when_outcome_has_failed_status() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let outcome = StepOutcome {
        last_node_status: Some((node.id.clone(), NodeStatus::Failed("bad thing".into()))),
        has_error: IsPredicate::no(),
        error_message: None,
    };
    let result = evaluate_gate(&node, &outcome);
    assert!(!result.passed);
    assert_eq!(result.reason.as_deref(), Some("bad thing"));
}

/// Verifies that `evaluate_gate` checks the error flag before the node status,
/// so an error supersedes an otherwise-passing status.
#[test]
fn gate_fails_when_outcome_has_error_flag() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let outcome = StepOutcome {
        last_node_status: Some((node.id.clone(), NodeStatus::Done)),
        has_error: IsPredicate::yes(),
        error_message: Some(OutputText::new("crash")),
    };
    let result = evaluate_gate(&node, &outcome);
    assert!(!result.passed);
    assert_eq!(result.reason.as_deref(), Some("crash"));
}

/// Verifies that `evaluate_gate` falls back to `"executor error"` when an
/// error flag is set without a specific message.
#[test]
fn gate_fails_with_generic_executor_error_when_message_missing() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let outcome = StepOutcome {
        last_node_status: Some((node.id.clone(), NodeStatus::Done)),
        has_error: IsPredicate::yes(),
        error_message: None,
    };
    let result = evaluate_gate(&node, &outcome);
    assert!(!result.passed);
    assert_eq!(result.reason.as_deref(), Some("executor error"));
}

/// Verifies that `StepOutcome::error_message` uses `OutputText` rather than a bare `String`.
///
/// Expected outcome: the runtime type name of the field matches `Option<OutputText>`.
#[test]
fn step_outcome_error_message_uses_output_text() {
    let outcome = StepOutcome::default();
    assert_eq!(
        std::any::type_name_of_val(&outcome.error_message),
        std::any::type_name::<Option<OutputText>>(),
        "StepOutcome::error_message should use Option<OutputText>"
    );
}

/// Verifies that `PhaseGateResult::reason` uses `OutputText` rather than a bare `String`.
///
/// Expected outcome: the runtime type name of the evaluated reason matches `Option<OutputText>`.
#[test]
fn phase_gate_result_reason_uses_output_text() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let result = evaluate_gate(&node, &StepOutcome::default());
    assert_eq!(
        std::any::type_name_of_val(&result.reason),
        std::any::type_name::<Option<OutputText>>(),
        "PhaseGateResult::reason should use Option<OutputText>"
    );
}

/// Verifies that `evaluate_gate` returns `passed: IsPredicate::no()` with a descriptive
/// reason when no `PlanNodeUpdate` was received (default `StepOutcome`).
#[test]
fn gate_fails_when_no_update_received() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let outcome = StepOutcome::default();
    let result = evaluate_gate(&node, &outcome);
    assert!(!result.passed);
    assert!(
        result
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("no PlanNodeUpdate"),
        "reason should mention missing PlanNodeUpdate"
    );
}

/// Verifies that `evaluate_gate` returns `passed: IsPredicate::no()` when the update
/// carries a different node id than the expected node.
#[test]
fn gate_fails_when_update_is_for_different_node() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let other_id = PlanNodeId::new("other-node");
    let outcome = StepOutcome {
        last_node_status: Some((other_id, NodeStatus::Done)),
        has_error: IsPredicate::no(),
        error_message: None,
    };
    let result = evaluate_gate(&node, &outcome);
    assert!(!result.passed);
    assert!(
        result
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("different node"),
        "reason should mention different node"
    );
}

/// Verifies that non-terminal node states fail the gate with an unexpected-status reason.
#[test]
fn gate_fails_when_status_is_pending() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let outcome = StepOutcome {
        last_node_status: Some((node.id.clone(), NodeStatus::Pending)),
        has_error: IsPredicate::no(),
        error_message: None,
    };
    let result = evaluate_gate(&node, &outcome);
    assert!(!result.passed);
    assert_eq!(
        result.reason.as_deref(),
        Some("unexpected node status: Pending")
    );
}

/// Verifies that in-progress node states fail the gate with an unexpected-status reason.
#[test]
fn gate_fails_when_status_is_in_progress() {
    let node = PlanNode::new_leaf("step-1", "Step 1", "steps/step-1.md");
    let outcome = StepOutcome {
        last_node_status: Some((node.id.clone(), NodeStatus::InProgress)),
        has_error: IsPredicate::no(),
        error_message: None,
    };
    let result = evaluate_gate(&node, &outcome);
    assert!(!result.passed);
    assert_eq!(
        result.reason.as_deref(),
        Some("unexpected node status: InProgress")
    );
}
