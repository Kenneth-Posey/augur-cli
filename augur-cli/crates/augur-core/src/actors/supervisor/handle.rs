//! `SupervisorHandle` - cloneable handle to a running `SupervisorActor`.
//!
//! Exposes command sending and event subscription. Only `wiring.rs`
//! constructs this handle.

use super::commands::SupervisorCmd;
use augur_domain::domain::channels::SUPERVISOR_OUTPUT_CAPACITY;
use augur_domain::domain::types::SupervisorEvent;
use augur_domain::domain::{GoalText, PlanNode, PlanNodeId};
use tokio::sync::{broadcast, mpsc};

/// Cloneable handle to a running `SupervisorActor`.
///
/// Wraps the command sender and event broadcast sender. All clones share the
/// same underlying channels. The TUI subscribes to events; the user triggers
/// `start_plan` to begin execution.
#[derive(Clone)]
pub struct SupervisorHandle {
    cmd_tx: mpsc::Sender<SupervisorCmd>,
    event_tx: broadcast::Sender<SupervisorEvent>,
}

impl SupervisorHandle {
    /// Construct a handle from raw channel endpoints.
    ///
    /// Called only by `SupervisorActor::spawn`.
    pub(super) fn new(
        cmd_tx: mpsc::Sender<SupervisorCmd>,
        event_tx: broadcast::Sender<SupervisorEvent>,
    ) -> Self {
        SupervisorHandle { cmd_tx, event_tx }
    }

    /// Start meta-planning and executing a plan for the given high-level goal.
    ///
    /// The supervisor builds the plan tree by sending the goal to the executor
    /// in meta-planning mode, then dispatches leaf steps for execution.
    #[tracing::instrument(skip(self), level = "info")]
    pub async fn start_plan(&self, goal: GoalText) {
        let cmd = SupervisorCmd::StartPlan { goal };
        if self.cmd_tx.send(cmd).await.is_err() {
            tracing::warn!("SupervisorHandle::start_plan: actor has stopped");
        }
    }

    /// Pause execution after the current step completes.
    #[tracing::instrument(skip(self), level = "info")]
    pub async fn pause(&self) {
        if self.cmd_tx.send(SupervisorCmd::Pause).await.is_err() {
            tracing::warn!("SupervisorHandle::pause: actor has stopped");
        }
    }

    /// Resume execution after a `Pause`.
    #[tracing::instrument(skip(self), level = "info")]
    pub async fn resume(&self) {
        if self.cmd_tx.send(SupervisorCmd::Resume).await.is_err() {
            tracing::warn!("SupervisorHandle::resume: actor has stopped");
        }
    }

    /// Cancel the current plan execution.
    #[tracing::instrument(skip(self), level = "info")]
    pub async fn cancel_plan(&self) {
        if self.cmd_tx.send(SupervisorCmd::CancelPlan).await.is_err() {
            tracing::warn!("SupervisorHandle::cancel_plan: actor has stopped");
        }
    }

    /// Inject a new step node as a child of `parent_id` in the active plan.
    #[tracing::instrument(skip(self, node), level = "info")]
    pub async fn inject_step(&self, parent_id: PlanNodeId, node: PlanNode) {
        let cmd = SupervisorCmd::InjectStep { parent_id, node };
        if self.cmd_tx.send(cmd).await.is_err() {
            tracing::warn!("SupervisorHandle::inject_step: actor has stopped");
        }
    }

    /// Subscribe to the supervisor event broadcast channel.
    ///
    /// Returns a fresh receiver starting from the next emitted event.
    /// The TUI plan panel calls this once at startup.
    pub fn subscribe_events(&self) -> broadcast::Receiver<SupervisorEvent> {
        self.event_tx.subscribe()
    }

    /// Send a graceful stop signal to the actor.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(SupervisorCmd::Stop);
    }
}

/// Create a broadcast sender for the supervisor event channel.
///
/// Called by `SupervisorActor::spawn`. The sender is stored in the handle;
/// subscribers call `subscribe_events` on the handle.
pub(super) fn make_event_channel() -> broadcast::Sender<SupervisorEvent> {
    let (tx, _) = broadcast::channel(*SUPERVISOR_OUTPUT_CAPACITY);
    tx
}
