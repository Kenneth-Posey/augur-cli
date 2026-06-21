//! Public handle for reading state snapshots and sending commands to the TUI spinner actor.

use super::tui_spinner_ops::{SpinnerCmd, SpinnerState, SpinnerTarget};
use augur_domain::domain::string_newtypes::{StatusLabel, StringNewtype};
use tokio::sync::{mpsc, watch};

/// Handle to a running TUI spinner actor task.
///
/// Provides a watch-channel snapshot of the current spinner state and a
/// command sender for start/stop control. No shared mutable state -
/// reads are watch-channel borrows; writes are mpsc sends.
#[derive(Clone)]
pub struct TuiSpinnerHandle {
    pub(crate) tx: mpsc::Sender<SpinnerCmd>,
    pub(crate) state_rx: watch::Receiver<SpinnerState>,
}

impl TuiSpinnerHandle {
    /// Create a handle. Called only by `tui_spinner::actor::spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<SpinnerCmd>,
        state_rx: watch::Receiver<SpinnerState>,
    ) -> Self {
        TuiSpinnerHandle { tx, state_rx }
    }

    /// Start the spinner for the given target, displaying the supplied label.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    #[allow(dead_code)]
    pub(crate) fn start(&self, target: SpinnerTarget, label: StatusLabel) {
        let _ = self.tx.try_send(SpinnerCmd::Start {
            target,
            label: label.into_inner(),
        });
    }

    /// Stop the spinner for the given target.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    pub fn stop(&self, target: SpinnerTarget) {
        let _ = self.tx.try_send(SpinnerCmd::Stop(target));
    }

    /// Return the current spinner state by reading the watch-channel snapshot.
    ///
    /// This is a momentary borrow of the watch channel's internal cell - not
    /// shared mutable state. The value reflects whatever the actor last set.
    pub fn current_state(&self) -> SpinnerState {
        self.state_rx.borrow().clone()
    }

    /// Send a graceful shutdown signal to the spinner actor.
    ///
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(SpinnerCmd::Shutdown);
    }
}
