//! Fake `UserMessageConsumerHandle` for use in TUI unit tests.

use crate::actors::user_message_consumer::user_message_consumer_ops::UserMessageCmd;
use crate::actors::user_message_consumer::UserMessageConsumerHandle;
use tokio::sync::mpsc;

/// Builds a disconnected `UserMessageConsumerHandle` whose command
/// channel is never read.  Tests that construct `TuiHandles` directly need
/// a `user_message_consumer` field; this satisfies that requirement without
/// spawning a real actor.
pub fn fake_user_message_consumer_handle() -> UserMessageConsumerHandle {
    let (tx, _rx) = mpsc::channel(1);
    UserMessageConsumerHandle { tx }
}

/// Builds a `UserMessageConsumerHandle` paired with a live receiver.
///
/// Use this variant in tests that need to assert that `process_input` was
/// called: read the returned `mpsc::Receiver<UserMessageCmd>` after the
/// code under test has run.
pub fn observable_user_message_consumer_handle(
) -> (UserMessageConsumerHandle, mpsc::Receiver<UserMessageCmd>) {
    let (tx, rx) = mpsc::channel(16);
    (UserMessageConsumerHandle { tx }, rx)
}
