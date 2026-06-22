//! TUI chat-menu actor: owns chat-menu visibility and selection state.

use super::handle::TuiChatMenuHandle;
use super::tui_chat_menu_actor_ops as actor_ops;
use super::tui_chat_menu_ops::{ChatMenuCmd, ChatMenuState};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::OutputText;
use tokio::sync::{mpsc, watch};

/// Spawn the TUI chat-menu actor and return a join handle plus a `TuiChatMenuHandle`.
///
/// Creates a `watch::channel` seeded with the default `ChatMenuState`. Creates
/// an `mpsc::channel` with the given `capacity` for commands. The actor task
/// owns the `watch::Sender`; callers read snapshots via `TuiChatMenuHandle`.
pub fn spawn(capacity: Count) -> (tokio::task::JoinHandle<()>, TuiChatMenuHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(capacity.inner());
    let (state_tx, state_rx) = watch::channel(ChatMenuState::builder().build());
    let handle = TuiChatMenuHandle::new(cmd_tx, state_rx);
    let join = tokio::spawn(run(cmd_rx, state_tx));
    (join, handle)
}

/// Actor task loop: processes chat-menu commands and publishes state updates.
///
/// Exits on `ChatMenuCmd::Shutdown` or when the command channel is closed.
async fn run(mut rx: mpsc::Receiver<ChatMenuCmd>, state_tx: watch::Sender<ChatMenuState>) {
    loop {
        match rx.recv().await {
            None | Some(ChatMenuCmd::Shutdown) => break,
            Some(ChatMenuCmd::Show(items)) => {
                actor_ops::apply_show(&state_tx, items.into_iter().map(OutputText::from).collect());
            }
            Some(ChatMenuCmd::Hide) => {
                actor_ops::apply_hide(&state_tx);
            }
            Some(ChatMenuCmd::SetAction(action)) => {
                actor_ops::apply_set_action(&state_tx, action);
            }
        }
    }
}
