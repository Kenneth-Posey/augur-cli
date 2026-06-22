//! TUI main feed panel actor: aggregates main agent, ask-panel, and orchestrator events.
//!
//! Accepts [`MainFeedCmd::Agent`], [`MainFeedCmd::Ask`], and
//! [`MainFeedCmd::Orchestrator`] commands and forwards them as a unified
//! [`MainFeedItem`] stream for the TUI main conversation panel.
//! Also maintains a [`MainFeedState`] watch channel so the TUI runtime can
//! read a snapshot of accumulated lines without holding a live borrow.

use super::handle::TuiMainFeedPanelHandle;
use super::tui_main_feed_panel_actor_ops as actor_ops;
use super::tui_main_feed_panel_ops::{MainFeedCmd, MainFeedItem, MainFeedState};
use crate::domain::tui_state::OutputLine;
use augur_domain::domain::types::AgentOutput;
use tokio::sync::{mpsc, watch};

/// Configuration for spawning the TUI main feed panel actor.
///
/// `unified_tx` is the sink for all forwarded feed items. `capacity` sets the
/// command channel buffer size; use `TUI_FEED_CAPACITY.inner()` at call sites.
pub struct TuiMainFeedConfig {
    /// Sink channel for the unified main feed item stream.
    pub unified_tx: mpsc::Sender<MainFeedItem>,
    /// Command channel buffer capacity.
    pub capacity: usize,
}

/// Spawn the TUI main feed panel actor and return a join handle plus a `TuiMainFeedPanelHandle`.
///
/// Creates a `watch::channel` seeded with an empty `MainFeedState` and an
/// `mpsc::channel` with `config.capacity` for commands. The actor task loops
/// over commands, updates accumulated line state, and forwards feed items to
/// `config.unified_tx`. Returns `(JoinHandle, TuiMainFeedPanelHandle)`.
pub fn spawn(config: TuiMainFeedConfig) -> (tokio::task::JoinHandle<()>, TuiMainFeedPanelHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(config.capacity);
    let (state_tx, state_rx) = watch::channel(MainFeedState::default());
    let handle = TuiMainFeedPanelHandle::new(cmd_tx, state_rx);
    let join = tokio::spawn(run(cmd_rx, config.unified_tx, state_tx));
    (join, handle)
}

/// Actor task loop: forwards main feed items and maintains accumulated line state.
///
/// Exits on [`MainFeedCmd::Shutdown`] or when the command channel is closed.
/// After each command the updated `MainFeedState` is published to the watch
/// channel. Errors sending to `unified_tx` are silently ignored.
async fn run(
    mut rx: mpsc::Receiver<MainFeedCmd>,
    unified_tx: mpsc::Sender<MainFeedItem>,
    state_tx: watch::Sender<MainFeedState>,
) {
    let mut lines: Vec<OutputLine> = Vec::new();
    loop {
        match rx.recv().await {
            None | Some(MainFeedCmd::Shutdown) => break,
            Some(MainFeedCmd::Agent(item)) => {
                accumulate_agent_output(&mut lines, &item);
                let _ = unified_tx.send(MainFeedItem::AgentOut(item)).await;
            }
            Some(MainFeedCmd::Ask(item)) => {
                accumulate_agent_output(&mut lines, &item);
                let _ = unified_tx.send(MainFeedItem::AskOut(item)).await;
            }
            Some(MainFeedCmd::Orchestrator(ev)) => {
                let _ = unified_tx.send(MainFeedItem::OrchestratorEvent(ev)).await;
            }
        }
        state_tx.send_replace(MainFeedState::builder().lines(lines.clone()).build());
    }
}

/// Update the accumulated lines vector from an `AgentOutput` event.
///
/// `Token` chunks are appended to the last line (or start a new one). `Error`
/// chunks are pushed as a distinct error-styled line. All other variants are
/// silently ignored - they carry no display text.
fn accumulate_agent_output(lines: &mut Vec<OutputLine>, item: &AgentOutput) {
    match item {
        AgentOutput::Token(text) => actor_ops::append_token(lines, text.clone()),
        AgentOutput::Error(text) => lines.push(OutputLine::error(text.clone())),
        _ => {}
    }
}
