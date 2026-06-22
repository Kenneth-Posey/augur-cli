//! Private helper operations for the TUI chat-menu actor.

use super::tui_chat_menu_ops::{ChatMenuAction, ChatMenuState};
use crate::domain::newtypes::IsVisible;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use tokio::sync::watch;

/// Show the chat menu with the supplied output text items.
pub(super) fn apply_show(state_tx: &watch::Sender<ChatMenuState>, items: Vec<OutputText>) {
    state_tx.send_replace(
        ChatMenuState::builder()
            .visible(IsVisible::yes())
            .items(items.into_iter().map(|item| item.into_inner()).collect())
            .build(),
    );
}

/// Hide the chat menu while preserving the current item list.
pub(super) fn apply_hide(state_tx: &watch::Sender<ChatMenuState>) {
    let current_items = state_tx.borrow().items.clone();
    state_tx.send_replace(
        ChatMenuState::builder()
            .visible(IsVisible::no())
            .items(current_items)
            .build(),
    );
}

/// Bind the selected menu action while preserving current visibility and items.
pub(super) fn apply_set_action(state_tx: &watch::Sender<ChatMenuState>, action: ChatMenuAction) {
    let current = state_tx.borrow().clone();
    state_tx.send_replace(
        ChatMenuState::builder()
            .visible(current.visible)
            .items(current.items)
            .selected_action(action)
            .build(),
    );
}
