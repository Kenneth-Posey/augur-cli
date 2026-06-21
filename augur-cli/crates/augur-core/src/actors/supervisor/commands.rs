//! Commands accepted by the `SupervisorActor` command channel.

use augur_domain::domain::{GoalText, PlanNode, PlanNodeId};

/// Commands sent to the running `SupervisorActor` via its command channel.
///
/// The supervisor processes commands sequentially. Only one plan may be active
/// at a time; sending `StartPlan` while one is running is silently ignored.
#[derive(Debug)]
pub enum SupervisorCmd {
    /// Start meta-planning and executing a plan for the given high-level goal.
    ///
    /// The supervisor constructs a `PlanTree` by sending the goal to the
    /// executor in meta-planning mode, then begins step execution.
    StartPlan { goal: GoalText },
    /// Pause execution after the current step completes.
    ///
    /// The supervisor stops dispatching new steps until `Resume` is received.
    Pause,
    /// Resume execution after a `Pause` command.
    Resume,
    /// Cancel the current plan execution immediately.
    ///
    /// The supervisor emits `SupervisorEvent::Failed` with reason "cancelled"
    /// and resets to idle. Steps already completed are not reversed.
    CancelPlan,
    /// Inject a new step node as a child of the given parent in the active plan.
    ///
    /// Used to add dynamically-generated steps during execution. No-op when
    /// there is no active plan or when `parent_id` is not found in the tree.
    InjectStep {
        /// The id of the existing node to attach the new step to.
        parent_id: PlanNodeId,
        /// The new step node to insert under `parent_id`.
        node: PlanNode,
    },
    /// Shut down the supervisor actor task.
    ///
    /// The task exits its command loop cleanly after this command is processed.
    Stop,
}
