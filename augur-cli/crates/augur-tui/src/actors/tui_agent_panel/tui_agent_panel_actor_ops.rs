//! Private helper operations for the TUI agent-panel actor.

use super::tui_agent_panel_ops::AgentPanelCmd;
use crate::domain::tui_state::{AgentFeedState, OutputLine};
use augur_domain::domain::string_newtypes::{AgentName, StringNewtype, TaskName};
use augur_domain::domain::types::AgentFeedOutput;
use tokio::sync::{mpsc, watch};

/// Actor task loop: forwards agent and tool feed items and maintains accumulated state.
///
/// Exits on [`AgentPanelCmd::Shutdown`] or when the command channel is closed.
/// After each command the updated `AgentFeedState` is published to the watch
/// channel. Errors sending to `unified_tx` are silently ignored.
pub(super) async fn run(
    mut rx: mpsc::Receiver<AgentPanelCmd>,
    unified_tx: mpsc::Sender<AgentFeedOutput>,
    state_tx: watch::Sender<AgentFeedState>,
) {
    let mut state = AgentFeedState::default();
    loop {
        match rx.recv().await {
            None | Some(AgentPanelCmd::Shutdown) => break,
            Some(AgentPanelCmd::AgentFeed(item)) => {
                apply_feed_output(&mut state, &item);
                let _ = unified_tx.send(item).await;
            }
            Some(AgentPanelCmd::ToolFeed(item)) => {
                apply_feed_output(&mut state, &item);
                let _ = unified_tx.send(item).await;
            }
        }
        state_tx.send_replace(state.clone());
    }
}

/// Apply a single `AgentFeedOutput` item to the accumulated `AgentFeedState`.
pub(super) fn apply_feed_output(state: &mut AgentFeedState, item: &AgentFeedOutput) {
    match item {
        AgentFeedOutput::StatusLine(text) => {
            state.output.push(OutputLine::plain(text.clone()));
        }
        AgentFeedOutput::ToolEventLine(text) => {
            state.output.push(OutputLine::tool_call(text.clone()));
        }
        AgentFeedOutput::TaskStarted { name, model } => {
            state.active_task = Some(agent_name_to_task_name(name));
            state.current_agent_model = model.clone();
        }
        AgentFeedOutput::TaskCompleted { .. } => {
            state.active_task = None;
        }
        AgentFeedOutput::TaskFailed { reason, .. } => {
            state.output.push(OutputLine::error(reason.clone()));
            state.active_task = None;
        }
        AgentFeedOutput::MessageBreak => {}
        AgentFeedOutput::Clear => {
            *state = AgentFeedState::default();
        }
    }

    fn agent_name_to_task_name(name: &AgentName) -> TaskName {
        TaskName::new(name.to_string())
    }
}
