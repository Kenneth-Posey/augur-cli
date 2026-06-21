//! Command and state types for the TUI dynamic controls actor.

use crate::domain::newtypes::IsVisible;
use crate::domain::string_newtypes::{ControlKey, ControlLabel};

/// A single runtime key hint displayed in the dynamic controls panel.
#[derive(Debug, Clone)]
pub struct ControlItem {
    /// The key label (e.g. `"q"`).
    pub key: ControlKey,
    /// The human-readable description (e.g. `"quit"`).
    pub label: ControlLabel,
}

/// Published state snapshot for the TUI dynamic controls panel.
#[derive(Debug, Clone)]
pub struct DynamicControlsState {
    /// The ordered list of key hints currently shown.
    pub controls: Vec<ControlItem>,
    /// Whether the dynamic controls panel is visible.
    pub visible: IsVisible,
}

impl Default for DynamicControlsState {
    fn default() -> Self {
        Self {
            controls: Vec::new(),
            visible: IsVisible::yes(),
        }
    }
}

/// Commands accepted by the TUI dynamic controls actor's mpsc channel.
#[derive(Debug)]
pub enum DynamicControlsCmd {
    /// Replace the full list of displayed key hints.
    SetControls(Vec<ControlItem>),
    /// Show or hide the dynamic controls panel.
    SetVisible(bool),
    /// Stop the actor task.
    Shutdown,
}
