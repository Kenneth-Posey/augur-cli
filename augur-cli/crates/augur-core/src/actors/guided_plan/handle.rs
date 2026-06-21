//! Handle for the `GuidedPlanActor`: command senders and event subscription.

use super::commands::GuidedPlanCmd;
use augur_domain::domain::guided_plan::{GuidedPlanConfig, GuidedPlanEvent};
use augur_domain::domain::FilePath;
use tokio::sync::{broadcast, mpsc};

/// Public handle for sending commands to and subscribing to events from the
/// `GuidedPlanActor`.
///
/// All `try_send` calls silently drop the command when the channel is full or
/// disconnected - the actor is best-effort for UI interactions. Consumers:
/// `wiring::run`, `TuiServiceHandles`, `key_dispatch::handle_submit`.
#[derive(Clone)]
pub struct GuidedPlanHandle {
    /// Sending half of the command channel.
    pub(crate) cmd_tx: mpsc::Sender<GuidedPlanCmd>,
    /// Broadcast channel for event subscriptions.
    pub(crate) event_tx: broadcast::Sender<GuidedPlanEvent>,
}

impl GuidedPlanHandle {
    /// Load a plan and start execution from phase 0.
    ///
    /// Sends `GuidedPlanCmd::Start` to the actor. The actor transitions phase 0
    /// to `InProgress` and emits `PhaseStatusChanged` for all phases.
    pub fn start(&self, config: GuidedPlanConfig, plan_path: FilePath) {
        let _ = self
            .cmd_tx
            .try_send(GuidedPlanCmd::Start { config, plan_path });
    }

    /// Confirm that the current phase is complete and begin hook execution.
    ///
    /// Sends `GuidedPlanCmd::ConfirmPhase`. The actor transitions to
    /// `AwaitingHooks` and runs the post-phase hook sequence.
    pub fn confirm_phase(&self) {
        let _ = self.cmd_tx.try_send(GuidedPlanCmd::ConfirmPhase);
    }

    /// Force-advance past a `NeedsRework` gate, bypassing remaining hooks.
    ///
    /// Sends `GuidedPlanCmd::ForceAdvance`. The actor logs a warning and
    /// transitions the phase to `Complete` without re-running hooks.
    pub fn force_advance(&self) {
        let _ = self.cmd_tx.try_send(GuidedPlanCmd::ForceAdvance);
    }

    /// Notify the actor that conversation compaction has finished.
    ///
    /// Sends `GuidedPlanCmd::CompactionDone`. If the actor was blocked on a
    /// compaction wait, it unblocks and advances to the next phase.
    pub fn compaction_done(&self) {
        let _ = self.cmd_tx.try_send(GuidedPlanCmd::CompactionDone);
    }

    /// Subscribe to the `GuidedPlanEvent` broadcast channel.
    ///
    /// Returns a new receiver. Multiple receivers may coexist; each gets a copy
    /// of every event. Used by the TUI actor to drive UI updates.
    pub fn subscribe(&self) -> broadcast::Receiver<GuidedPlanEvent> {
        self.event_tx.subscribe()
    }

    /// Shut down the actor loop.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(GuidedPlanCmd::Shutdown);
    }
}
