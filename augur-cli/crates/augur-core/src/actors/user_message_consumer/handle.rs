//! UserMessageConsumerHandle: fire-and-forget client for the user message consumer actor.

use super::user_message_consumer_ops::UserMessageCmd;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use tokio::sync::mpsc;

/// Fire-and-forget handle to the running user message consumer actor.
///
/// Callers submit raw input strings for classification and routing without
/// waiting for the operation to complete. Dropping all clones causes the
/// actor's receiver to close.
pub struct UserMessageConsumerHandle {
    pub(crate) tx: mpsc::Sender<UserMessageCmd>,
}

impl UserMessageConsumerHandle {
    /// Enqueue a raw input string for classification and routing.
    ///
    /// Sends without blocking the caller. Silently drops the message if the
    /// actor channel is full or the actor has stopped.
    #[allow(dead_code)]
    pub(crate) fn process_input(&self, text: OutputText) {
        let _ = self
            .tx
            .try_send(UserMessageCmd::ProcessInput(text.into_inner()));
    }

    /// Send a graceful shutdown signal to the user message consumer actor.
    ///
    /// The actor will exit its receive loop after processing this command.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(UserMessageCmd::Shutdown);
    }
}
