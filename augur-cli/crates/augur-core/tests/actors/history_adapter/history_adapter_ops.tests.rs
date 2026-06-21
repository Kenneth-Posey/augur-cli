//! Unit tests for history adapter ops: pure command-to-feed-message conversion.

use augur_core::actors::history_adapter::history_adapter_ops::{
    to_history_entry, HistoryAdapterCmd,
};
use augur_domain::domain::feeds::HistoryFeedMessage;
use augur_domain::domain::types::Message;

/// Verifies that `RecordUser` produces `Some(HistoryFeedMessage::UserEntry)`.
#[test]
fn test_to_history_entry_user_variant() {
    let msg = Message::user("hello from user");
    let cmd = HistoryAdapterCmd::RecordUser(msg.clone());
    let result = to_history_entry(&cmd);
    match result {
        Some(HistoryFeedMessage::UserEntry(m)) => {
            assert_eq!(m.content, msg.content);
        }
        other => panic!("expected Some(UserEntry), got {other:?}"),
    }
}

/// Verifies that `RecordLlm` produces `Some(HistoryFeedMessage::LlmEntry)`.
#[test]
fn test_to_history_entry_llm_variant() {
    let msg = Message::assistant("hello from llm");
    let cmd = HistoryAdapterCmd::RecordLlm(msg.clone());
    let result = to_history_entry(&cmd);
    match result {
        Some(HistoryFeedMessage::LlmEntry(m)) => {
            assert_eq!(m.content, msg.content);
        }
        other => panic!("expected Some(LlmEntry), got {other:?}"),
    }
}

/// Verifies that `Shutdown` produces `None`.
#[test]
fn test_to_history_entry_shutdown_is_none() {
    let cmd = HistoryAdapterCmd::Shutdown;
    let result = to_history_entry(&cmd);
    assert!(result.is_none(), "Shutdown must produce None");
}
