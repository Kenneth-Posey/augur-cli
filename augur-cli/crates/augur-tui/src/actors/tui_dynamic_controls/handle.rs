//! Public handle for reading state snapshots and sending commands to the TUI dynamic controls actor.

use super::tui_dynamic_controls_ops::{ControlItem, DynamicControlsCmd, DynamicControlsState};
use tokio::sync::{mpsc, watch};

/// Semantic visibility toggle for the dynamic controls panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ControlsVisibility {
    Visible,
    Hidden,
}

impl From<bool> for ControlsVisibility {
    fn from(value: bool) -> Self {
        if value {
            Self::Visible
        } else {
            Self::Hidden
        }
    }
}

impl ControlsVisibility {}

impl From<ControlsVisibility> for bool {
    fn from(value: ControlsVisibility) -> Self {
        matches!(value, ControlsVisibility::Visible)
    }
}

/// Handle to a running TUI dynamic controls actor task.
///
/// Provides a watch-channel snapshot of the current dynamic controls state and
/// a command sender for updating controls and visibility. No shared mutable
/// state - reads are watch-channel borrows; writes are mpsc sends.
#[derive(Clone)]
pub struct TuiDynamicControlsHandle {
    pub(crate) tx: mpsc::Sender<DynamicControlsCmd>,
    pub(crate) state_rx: watch::Receiver<DynamicControlsState>,
}

impl TuiDynamicControlsHandle {
    /// Create a handle. Called only by `tui_dynamic_controls::actor::spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<DynamicControlsCmd>,
        state_rx: watch::Receiver<DynamicControlsState>,
    ) -> Self {
        TuiDynamicControlsHandle { tx, state_rx }
    }

    /// Replace the full list of displayed key hints.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    pub fn set_controls(&self, controls: Vec<ControlItem>) {
        let _ = self.tx.try_send(DynamicControlsCmd::SetControls(controls));
    }

    /// Show or hide the dynamic controls panel.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    #[allow(dead_code)]
    pub(crate) fn set_visible(&self, visible: ControlsVisibility) {
        let _ = self
            .tx
            .try_send(DynamicControlsCmd::SetVisible(bool::from(visible)));
    }

    /// Return the current dynamic controls state by reading the watch-channel snapshot.
    ///
    /// This is a momentary borrow of the watch channel's internal cell - not
    /// shared mutable state. The value reflects whatever the actor last set.
    pub fn current_state(&self) -> DynamicControlsState {
        self.state_rx.borrow().clone()
    }

    /// Send a graceful shutdown signal to the dynamic controls actor.
    ///
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(DynamicControlsCmd::Shutdown);
    }
}
