//! TUI spinner actor: owns spinner animation state and label text.

use super::handle::TuiSpinnerHandle;
use super::tui_spinner_actor_ops as actor_ops;
use super::tui_spinner_ops::{SpinnerCmd, SpinnerState, SpinnerTarget};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::StatusLabel;
use tokio::sync::{mpsc, watch};

/// Spawn the TUI spinner actor and return a join handle plus a `TuiSpinnerHandle`.
///
/// Creates a `watch::channel` seeded with an inactive `SpinnerState` targeting
/// `MainConversation`. Creates an `mpsc::channel` with the given `capacity` for
/// commands. The actor task owns the `watch::Sender`; callers read snapshots via
/// `TuiSpinnerHandle`.
pub fn spawn(capacity: Count) -> (tokio::task::JoinHandle<()>, TuiSpinnerHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(capacity.inner());
    let initial = SpinnerState::builder()
        .target(SpinnerTarget::MainConversation)
        .build();
    let (state_tx, state_rx) = watch::channel(initial);
    let handle = TuiSpinnerHandle::new(cmd_tx, state_rx);
    let join = tokio::spawn(run(cmd_rx, state_tx));
    (join, handle)
}

/// Actor task loop: processes spinner commands and publishes state updates.
///
/// Exits on `SpinnerCmd::Shutdown` or when the command channel is closed.
async fn run(mut rx: mpsc::Receiver<SpinnerCmd>, state_tx: watch::Sender<SpinnerState>) {
    loop {
        match rx.recv().await {
            None | Some(SpinnerCmd::Shutdown) => break,
            Some(SpinnerCmd::Start { target, label }) => {
                actor_ops::apply_start(&state_tx, target, StatusLabel::from(label));
            }
            Some(SpinnerCmd::Stop(target)) => {
                actor_ops::apply_stop(&state_tx, target);
            }
        }
    }
}
