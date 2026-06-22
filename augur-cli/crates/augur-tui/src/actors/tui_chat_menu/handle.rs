//! Public handle for reading state snapshots and sending commands to the TUI chat-menu actor.

use super::tui_chat_menu_ops::{ChatMenuAction, ChatMenuCmd, ChatMenuState};
use augur_domain::domain::StringNewtype;
use augur_domain::domain::string_newtypes::OutputText;
use tokio::sync::{mpsc, watch};

/// Handle to a running `TuiChatMenuActor` task.
///
/// Provides a watch-channel snapshot of the current chat-menu state and a
/// command sender for visibility and action changes. No shared mutable state -
/// reads are watch-channel borrows; writes are mpsc sends.
#[derive(Clone)]
pub struct TuiChatMenuHandle {
    tx: mpsc::Sender<ChatMenuCmd>,
    state_rx: watch::Receiver<ChatMenuState>,
}

impl TuiChatMenuHandle {
    /// Create a handle. Called only by `tui_chat_menu::actor::spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<ChatMenuCmd>,
        state_rx: watch::Receiver<ChatMenuState>,
    ) -> Self {
        TuiChatMenuHandle { tx, state_rx }
    }

    /// Make the chat menu visible with the supplied item list.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    #[allow(dead_code)]
    pub(crate) fn show(&self, items: Vec<OutputText>) {
        let _ = self.tx.try_send(ChatMenuCmd::Show(
            items.into_iter().map(|item| item.into_inner()).collect(),
        ));
    }

    /// Hide the chat menu and clear the pending action.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    pub fn hide(&self) {
        let _ = self.tx.try_send(ChatMenuCmd::Hide);
    }

    /// Bind an action to the current menu selection.
    ///
    /// Uses `try_send`; ignores errors if the actor queue is full or stopped.
    pub fn set_action(&self, action: ChatMenuAction) {
        let _ = self.tx.try_send(ChatMenuCmd::SetAction(action));
    }

    /// Return the current chat-menu state by reading the watch-channel snapshot.
    ///
    /// This is a momentary borrow of the watch channel's internal cell - not
    /// shared mutable state. The value reflects whatever the actor last set.
    pub fn current_state(&self) -> ChatMenuState {
        self.state_rx.borrow().clone()
    }

    /// Send a graceful shutdown signal to the chat-menu actor.
    ///
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(ChatMenuCmd::Shutdown);
    }
}
