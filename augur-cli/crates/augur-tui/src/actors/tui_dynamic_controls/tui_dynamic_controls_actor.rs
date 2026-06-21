//! TUI dynamic controls actor: owns the runtime key-hint panel state.

use super::handle::TuiDynamicControlsHandle;
use super::tui_dynamic_controls_actor_ops as actor_ops;
use super::tui_dynamic_controls_ops::{DynamicControlsCmd, DynamicControlsState};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use tokio::sync::{mpsc, watch};

/// Spawn the TUI dynamic controls actor and return a join handle plus a
/// `TuiDynamicControlsHandle`.
///
/// Creates a `watch::channel` seeded with a default `DynamicControlsState`.
/// Creates an `mpsc::channel` with the given `capacity` for commands. The actor
/// task owns the `watch::Sender`; callers read snapshots via
/// `TuiDynamicControlsHandle`.
pub fn spawn(capacity: Count) -> (tokio::task::JoinHandle<()>, TuiDynamicControlsHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(capacity.inner());
    let (state_tx, state_rx) = watch::channel(DynamicControlsState::default());
    let handle = TuiDynamicControlsHandle::new(cmd_tx, state_rx);
    let join = tokio::spawn(run(cmd_rx, state_tx));
    (join, handle)
}

/// Actor task loop: processes dynamic controls commands and publishes state updates.
///
/// Exits on `DynamicControlsCmd::Shutdown` or when the command channel is closed.
async fn run(
    mut rx: mpsc::Receiver<DynamicControlsCmd>,
    state_tx: watch::Sender<DynamicControlsState>,
) {
    loop {
        match rx.recv().await {
            None | Some(DynamicControlsCmd::Shutdown) => break,
            Some(DynamicControlsCmd::SetControls(items)) => {
                actor_ops::apply_set_controls(&state_tx, items);
            }
            Some(DynamicControlsCmd::SetVisible(v)) => {
                actor_ops::apply_set_visible(&state_tx, v.into());
            }
        }
    }
}
