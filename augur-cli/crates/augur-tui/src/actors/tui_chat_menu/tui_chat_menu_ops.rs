//! Command and state types for the TUI chat-menu actor.

use crate::domain::newtypes::IsVisible;

/// Actions bound to the current chat-menu selection.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatMenuAction {
    /// Submit the current selection.
    Submit,
    /// Cancel without applying a selection.
    Cancel,
    /// Select the item at the given index.
    Select(usize),
}

/// Published state snapshot for the TUI chat-menu panel.
#[derive(Debug, Clone, Default, bon::Builder)]
pub struct ChatMenuState {
    /// Whether the chat menu panel is currently visible.
    #[builder(default)]
    pub visible: IsVisible,
    /// Ordered list of items displayed in the menu.
    #[builder(default)]
    pub items: Vec<String>,
    /// Action bound to the currently selected menu item, if any.
    pub selected_action: Option<ChatMenuAction>,
}

/// Commands accepted by the TUI chat-menu actor's mpsc channel.
#[derive(Debug)]
pub enum ChatMenuCmd {
    /// Make the menu visible with the supplied item list.
    Show(Vec<String>),
    /// Hide the menu and clear the pending action.
    Hide,
    /// Bind an action to the current selection.
    SetAction(ChatMenuAction),
    /// Stop the actor task.
    Shutdown,
}
