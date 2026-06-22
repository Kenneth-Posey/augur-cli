//! TuiAskPanelHandle: client for the TUI ask panel actor.

use super::tui_ask_panel_ops::AskPanelCmd;
use crate::domain::tui_state::{AskPanelState, OutputLine};
use augur_domain::domain::newtypes::{NumericNewtype, ScrollOffset};
use tokio::sync::{mpsc, watch};

/// Signed scroll delta measured in lines (positive = down, negative = up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScrollDelta(i64);

impl From<i64> for ScrollDelta {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl ScrollDelta {
    /// Apply this delta to a scroll offset, clamped at zero.
    pub(super) fn apply_to(self, current: ScrollOffset) -> ScrollOffset {
        let current = current.inner() as i64;
        let new_val = (current + self.0).max(0) as usize;
        ScrollOffset::of(new_val)
    }
}

/// Semantic state for the ask-panel thinking indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThinkingState {
    Thinking,
    Idle,
}

impl From<bool> for ThinkingState {
    fn from(value: bool) -> Self {
        if value { Self::Thinking } else { Self::Idle }
    }
}

impl ThinkingState {
    fn is_thinking(self) -> bool {
        matches!(self, Self::Thinking)
    }
}

/// Handle to a running TUI ask panel actor task.
///
/// Provides a watch-channel snapshot of the current ask panel state and a
/// command sender for all panel operations. No shared mutable state -
/// reads are watch-channel borrows; writes are mpsc sends.
#[derive(Clone)]
pub struct TuiAskPanelHandle {
    tx: mpsc::Sender<AskPanelCmd>,
    state_rx: watch::Receiver<Option<AskPanelState>>,
}

impl TuiAskPanelHandle {
    /// Create a handle. Called only by `tui_ask_panel::actor::spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<AskPanelCmd>,
        state_rx: watch::Receiver<Option<AskPanelState>>,
    ) -> Self {
        TuiAskPanelHandle { tx, state_rx }
    }

    /// Return the current ask panel state by reading the watch-channel snapshot.
    ///
    /// Returns `None` when the panel is closed, `Some(state)` when it is open.
    /// This is a momentary borrow of the watch channel's internal cell.
    pub fn current_state(&self) -> Option<AskPanelState> {
        self.state_rx.borrow().clone()
    }

    /// Clone the watch receiver so the TUI runtime can subscribe to state updates.
    ///
    /// Returns a new `watch::Receiver<Option<AskPanelState>>` tracking the same actor.
    pub fn state_rx(&self) -> watch::Receiver<Option<AskPanelState>> {
        self.state_rx.clone()
    }

    /// Open the ask panel.
    ///
    /// No-op if the panel is already open. Uses `try_send`; ignores errors.
    pub fn open(&self) {
        let _ = self.tx.try_send(AskPanelCmd::Open);
    }

    /// Close the ask panel and clear its state.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    pub fn close(&self) {
        let _ = self.tx.try_send(AskPanelCmd::Close);
    }

    /// Inject a history snapshot into the ask panel output.
    ///
    /// Inputs: `lines` - the display lines to prepend to the ask panel output.
    /// No-op if the panel is closed. Uses `try_send`; ignores errors.
    pub fn seed_history(&self, lines: Vec<OutputLine>) {
        let _ = self.tx.try_send(AskPanelCmd::SeedHistory(lines));
    }

    /// Append a single display line to the ask panel output.
    ///
    /// Inputs: `line` - the `OutputLine` to append.
    /// No-op if the panel is closed. Uses `try_send`; ignores errors.
    pub fn append_line(&self, line: OutputLine) {
        let _ = self.tx.try_send(AskPanelCmd::AppendLine(line));
    }

    /// Scroll the ask panel by `delta` lines (positive = down, negative = up).
    ///
    /// Clamped at zero. No-op if the panel is closed.
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    #[allow(dead_code)]
    pub(crate) fn scroll(&self, delta: ScrollDelta) {
        let _ = self.tx.try_send(AskPanelCmd::Scroll(delta.0));
    }

    /// Set the thinking indicator.
    ///
    /// Inputs: `val` - `true` while the ask actor is processing a turn.
    /// No-op if the panel is closed. Uses `try_send`; ignores errors.
    #[allow(dead_code)]
    pub(crate) fn set_thinking(&self, val: ThinkingState) {
        let _ = self
            .tx
            .try_send(AskPanelCmd::SetThinking(val.is_thinking()));
    }

    /// Send a graceful shutdown signal to the ask panel actor.
    ///
    /// The actor will exit its run loop after receiving this command.
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(AskPanelCmd::Shutdown);
    }
}
