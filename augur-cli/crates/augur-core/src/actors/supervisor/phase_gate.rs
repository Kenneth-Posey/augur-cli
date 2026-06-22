//! Phase gate logic for evaluating whether an executor step succeeded.
//!
//! `evaluate_gate` is a pure function that maps a `StepOutcome` and the
//! expected `PlanNode` onto a `PhaseGateResult`. It is kept free of I/O so
//! it can be unit-tested without spawning actors.

use augur_domain::domain::OutputText;
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::plan_tree::{NodeStatus, PlanNode, PlanNodeId};

// ── StepOutcome ───────────────────────────────────────────────────────────────

/// Accumulated observations from draining an executor's output for one step.
///
/// Built incrementally by `drain_step_output` in `actor.rs`, then passed to
/// `evaluate_gate` to decide success or failure.
#[derive(Clone, Debug, Default, bon::Builder)]
pub struct StepOutcome {
    /// The last `PlanNodeUpdate` status update seen during this step's drain.
    ///
    /// `None` if no `AgentOutput::PlanNodeUpdate` was observed at all.
    pub last_node_status: Option<(PlanNodeId, NodeStatus)>,
    /// Whether any `AgentOutput` variant indicating an executor error was seen.
    pub has_error: IsPredicate,
    /// Optional human-readable error message from the first error event seen.
    pub error_message: Option<OutputText>,
}

// ── PhaseGateResult ───────────────────────────────────────────────────────────

/// The evaluation result returned by `evaluate_gate`.
///
/// Callers branch on `passed`: when false, `reason` contains a human-readable
/// explanation suitable for `SupervisorEvent::StepFailed.reason`.
#[derive(Debug)]
pub struct PhaseGateResult {
    /// `true` when the step completed successfully.
    pub passed: IsPredicate,
    /// Failure reason; always `None` when `passed` is `true`.
    pub reason: Option<OutputText>,
}

// ── evaluate_gate ─────────────────────────────────────────────────────────────

/// Evaluates whether a step succeeded by inspecting `outcome` against `node`.
///
/// Call context: called immediately after `drain_step_output` returns, before
/// `complete_step` or `fail_step` is invoked.
///
/// Decision order (error flag is checked first):
/// 1. `has_error` → fail with `error_message` or generic message.
/// 2. `last_node_status` is `None` → fail with "no PlanNodeUpdate received".
/// 3. Status node id ≠ `node.id` → fail with "different node" message.
/// 4. `NodeStatus::Done` → pass.
/// 5. `NodeStatus::Failed(reason)` → fail with that reason.
/// 6. Any other status (`Pending`, `InProgress`) → fail with "unexpected" message.
pub fn evaluate_gate(node: &PlanNode, outcome: &StepOutcome) -> PhaseGateResult {
    let is_error = outcome.has_error;
    match is_error.0 {
        true => PhaseGateResult {
            passed: IsPredicate::no(),
            reason: Some(
                outcome
                    .error_message
                    .clone()
                    .unwrap_or_else(|| OutputText::from("executor error")),
            ),
        },
        false => evaluate_node_status(node, outcome),
    }
}

/// Evaluates the node-status portion of the gate (no error present).
fn evaluate_node_status(node: &PlanNode, outcome: &StepOutcome) -> PhaseGateResult {
    let Some((id, status)) = &outcome.last_node_status else {
        return missing_update_failure();
    };
    if id != &node.id {
        return wrong_node_failure(id);
    }
    evaluate_recorded_status(status)
}

fn missing_update_failure() -> PhaseGateResult {
    PhaseGateResult {
        passed: IsPredicate::no(),
        reason: Some(OutputText::from("no PlanNodeUpdate received for this step")),
    }
}

fn wrong_node_failure(id: &PlanNodeId) -> PhaseGateResult {
    PhaseGateResult {
        passed: IsPredicate::no(),
        reason: Some(OutputText::from(format!(
            "update arrived for different node: {}",
            id
        ))),
    }
}

fn evaluate_recorded_status(status: &NodeStatus) -> PhaseGateResult {
    match status {
        NodeStatus::Done => PhaseGateResult {
            passed: IsPredicate::yes(),
            reason: None,
        },
        NodeStatus::Failed(message) => PhaseGateResult {
            passed: IsPredicate::no(),
            reason: Some(OutputText::from(message.to_string())),
        },
        _ => PhaseGateResult {
            passed: IsPredicate::no(),
            reason: Some(OutputText::from(format!(
                "unexpected node status: {:?}",
                status
            ))),
        },
    }
}
