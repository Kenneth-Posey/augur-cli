//! TUI ask panel actor: manages ask-panel open/close state and output accumulation.
//!
//! Maintains an `Option<AskPanelState>` watch channel: `None` when the panel is
//! closed, `Some(state)` when it is open. Accepts [`AskPanelCmd`] commands to
//! control visibility, append lines, seed history, and toggle the thinking flag.

use super::handle::TuiAskPanelHandle;
use super::tui_ask_panel_actor_ops as actor_ops;
use super::tui_ask_panel_ops::AskPanelCmd;
use crate::domain::tui_state::{AskPanelState, OutputLine};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use tokio::sync::{mpsc, watch};

/// Spawn the TUI ask panel actor and return a join handle plus a `TuiAskPanelHandle`.
///
/// Creates a `watch::channel` seeded with `None` (panel closed) and an
/// `mpsc::channel` with `capacity` for commands. The actor task loops over
/// commands and publishes state updates after each one.
/// Returns `(JoinHandle, TuiAskPanelHandle)`.
pub fn spawn(capacity: Count) -> (tokio::task::JoinHandle<()>, TuiAskPanelHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(capacity.inner());
    let (state_tx, state_rx) = watch::channel::<Option<AskPanelState>>(None);
    let handle = TuiAskPanelHandle::new(cmd_tx, state_rx);
    let join = tokio::spawn(run(cmd_rx, state_tx));
    (join, handle)
}

/// Actor task loop: processes ask panel commands and publishes state updates.
///
/// Exits on `AskPanelCmd::Shutdown` or when the command channel is closed.
async fn run(mut rx: mpsc::Receiver<AskPanelCmd>, state_tx: watch::Sender<Option<AskPanelState>>) {
    loop {
        match rx.recv().await {
            None | Some(AskPanelCmd::Shutdown) => break,
            Some(cmd) => {
                let mut state = state_tx.borrow().clone();
                apply_ask_cmd(&mut state, cmd);
                state_tx.send_replace(state);
            }
        }
    }
}

/// Apply a single `AskPanelCmd` to the current `Option<AskPanelState>`.
///
/// Mutates in place. Commands that require an open panel are no-ops when
/// `state` is `None`.
fn apply_ask_cmd(state: &mut Option<AskPanelState>, cmd: AskPanelCmd) {
    match cmd {
        AskPanelCmd::Open => apply_open(state),
        AskPanelCmd::Close => *state = None,
        AskPanelCmd::SeedHistory(lines) => apply_seed_history(state, lines),
        AskPanelCmd::AppendLine(line) => apply_append_line(state, line),
        AskPanelCmd::Scroll(delta) => apply_scroll(state, delta),
        AskPanelCmd::SetThinking(val) => apply_set_thinking(state, val),
        AskPanelCmd::Shutdown => {}
    }
}

/// Open the ask panel if it is not already open.
fn apply_open(state: &mut Option<AskPanelState>) {
    if state.is_none() {
        *state = Some(AskPanelState::default());
    }
}

/// Seed the history lines into an open ask panel; no-op when closed.
fn apply_seed_history(state: &mut Option<AskPanelState>, lines: Vec<OutputLine>) {
    if let Some(s) = state.as_mut() {
        s.output.extend(lines);
        s.seeded = true.into();
    }
}

/// Append a single line to an open ask panel's output; no-op when closed.
fn apply_append_line(state: &mut Option<AskPanelState>, line: OutputLine) {
    if let Some(s) = state.as_mut() {
        s.output.push(line);
    }
}

/// Scroll an open ask panel by `delta` lines; no-op when closed.
fn apply_scroll(state: &mut Option<AskPanelState>, delta: i64) {
    actor_ops::apply_scroll(state, delta.into());
}

/// Set the thinking indicator on an open ask panel; no-op when closed.
fn apply_set_thinking(state: &mut Option<AskPanelState>, val: bool) {
    if let Some(s) = state.as_mut() {
        s.thinking = val.into();
    }
}
