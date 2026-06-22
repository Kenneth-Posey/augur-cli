//! Command and output types for the TUI main feed panel actor.

use crate::domain::tui_state::{OutputLine, OutputSelection};
use augur_core::domain::deterministic_orchestrator::DeterministicOrchestratorEvent;
use augur_domain::domain::newtypes::ScrollOffset;
use augur_domain::domain::types::AgentOutput;

/// Published watch-channel state for the TUI main feed panel.
///
/// Maintained by the actor run loop and sent on every command. Callers read a
/// snapshot via [`super::handle::TuiMainFeedPanelHandle::current_state`].
/// `scroll` and `selection` are managed externally (by the TUI runtime);
/// the actor only updates `lines` as feed items arrive.
#[derive(Default, Clone, bon::Builder)]
pub struct MainFeedState {
    /// Accumulated display lines from agent, ask, and orchestrator feeds.
    #[builder(default)]
    pub lines: Vec<OutputLine>,
    /// Scroll offset within the main feed panel. 0 = follow latest output.
    #[builder(default)]
    pub scroll: ScrollOffset,
    /// Active text selection, or `None` when no selection is in progress.
    pub selection: Option<OutputSelection>,
}

/// A unified item emitted on the main feed panel output channel.
///
/// Each variant wraps a typed event from one of the three feed sources:
/// the main agent, the ask panel, or the deterministic orchestrator.
#[derive(Debug, Clone)]
pub enum MainFeedItem {
    /// An item from the main agent output channel.
    AgentOut(AgentOutput),
    /// An item from the ask-panel output channel.
    AskOut(AgentOutput),
    /// An event from the deterministic orchestrator.
    OrchestratorEvent(DeterministicOrchestratorEvent),
}

/// Commands accepted by the TUI main feed panel actor.
///
/// `Agent`, `Ask`, and `Orchestrator` carry typed items and are forwarded to
/// the unified output channel. `Shutdown` stops the actor loop.
#[derive(Debug)]
pub enum MainFeedCmd {
    /// An `AgentOutput` item from the main agent.
    Agent(AgentOutput),
    /// An `AgentOutput` item from the ask panel.
    Ask(AgentOutput),
    /// An event from the deterministic orchestrator.
    Orchestrator(DeterministicOrchestratorEvent),
    /// Graceful shutdown: the actor exits its run loop.
    Shutdown,
}
