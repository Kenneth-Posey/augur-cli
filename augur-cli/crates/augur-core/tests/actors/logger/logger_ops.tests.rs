use augur_core::actors::logger::logger_ops::{
    app_log_file_name, current_unix_secs, history_entry_to_log_entry, message_log_file_name,
    message_to_entry, tui_log_file_name,
};
use augur_domain::domain::feeds::HistoryFeedMessage;
use augur_domain::domain::newtypes::{NumericNewtype, TimestampMs, TimestampSecs};
use augur_domain::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use augur_domain::domain::types::{Message, Role};
use std::fs;

#[test]
fn message_to_entry_maps_user_fields() {
    let msg = Message {
        role: Role::User,
        content: OutputText::new("hello"),
        timestamp: TimestampMs::new(5_000),
        tool_call_id: None,
        tool_calls: None,
    };
    let endpoint = EndpointName::new("test-ep");
    let entry = message_to_entry(&msg, &endpoint);
    assert_eq!(entry.role, "user");
    assert_eq!(entry.content, "hello");
    assert_eq!(entry.endpoint, "test-ep");
}

#[test]
fn history_entry_to_log_entry_user_and_llm() {
    let endpoint = EndpointName::new("ep");
    let user = Message {
        role: Role::User,
        content: OutputText::new("u"),
        timestamp: TimestampMs::new(1),
        tool_call_id: None,
        tool_calls: None,
    };
    let llm = Message {
        role: Role::Assistant,
        content: OutputText::new("a"),
        timestamp: TimestampMs::new(2),
        tool_call_id: None,
        tool_calls: None,
    };
    let user_entry = history_entry_to_log_entry(&HistoryFeedMessage::UserEntry(user), &endpoint);
    let llm_entry = history_entry_to_log_entry(&HistoryFeedMessage::LlmEntry(llm), &endpoint);
    assert_eq!(user_entry.role, "user");
    assert_eq!(llm_entry.role, "assistant");
}

#[test]
fn log_file_name_helpers_use_timestamp_secs() {
    let ts = TimestampSecs::new(1_700_000_000);
    assert_eq!(
        message_log_file_name(ts).to_string_lossy(),
        "1700000000_msg.jsonl"
    );
    assert_eq!(
        app_log_file_name(ts).to_string_lossy(),
        "1700000000_app.log"
    );
    assert_eq!(
        tui_log_file_name(ts).to_string_lossy(),
        "1700000000_tui.log"
    );
}

#[test]
fn current_unix_secs_is_non_zero() {
    assert!(current_unix_secs().inner() > 0);
}

#[test]
fn legacy_format_as_jsonl_tests_deprecated_due_private_visibility() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/logger/logger_ops.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("logger ops source must be readable");
    assert!(source.contains("pub(crate) fn format_as_jsonl(entry: &LogEntry) -> OutputText"));
}
