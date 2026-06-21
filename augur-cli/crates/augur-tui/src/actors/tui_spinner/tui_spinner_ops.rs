//! Command and state types for the TUI spinner actor.

use crate::domain::newtypes::IsActive;
use crate::domain::string_newtypes::SpinnerLabel;

/// Identifies which panel the spinner belongs to.
#[derive(Debug, Clone, PartialEq)]
pub enum SpinnerTarget {
    /// Spinner for the main conversation panel.
    MainConversation,
    /// Spinner for the agent panel.
    AgentPanel,
}

/// Published state snapshot for a TUI spinner.
#[derive(Debug, Clone, bon::Builder)]
pub struct SpinnerState {
    /// Whether the spinner is currently animating.
    #[builder(default = IsActive::no())]
    pub active: IsActive,
    /// Label text displayed alongside the spinner.
    #[builder(default = SpinnerLabel::from(""))]
    pub label: SpinnerLabel,
    /// Which panel this spinner belongs to.
    pub target: SpinnerTarget,
}

/// Commands accepted by the TUI spinner actor's mpsc channel.
#[derive(Debug)]
pub enum SpinnerCmd {
    /// Start the spinner for the given target with the supplied label.
    Start {
        /// Which panel to activate.
        target: SpinnerTarget,
        /// Label text to display.
        label: String,
    },
    /// Stop the spinner for the given target.
    Stop(SpinnerTarget),
    /// Stop the actor task.
    Shutdown,
}
