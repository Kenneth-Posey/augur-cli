//! Logger actor ops: pure log-entry formatting with no I/O.
//!
//! `LogEntry` is the normalized form for every message logged to the JSONL file.
//! `format_as_jsonl` serializes a single entry to a compact JSON line (no trailing newline).
//! `role_label` maps `Role` to its canonical lowercase label used in log output.

use augur_domain::domain::feeds::HistoryFeedMessage;
use augur_domain::domain::newtypes::{NumericNewtype, TimestampMs, TimestampSecs};
use augur_domain::domain::string_newtypes::{
    EndpointName, LogContent, OutputText, RoleLabel, StringNewtype,
};
use augur_domain::domain::types::{Message, Role};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub use augur_domain::LogCommand;

/// A single normalized log entry ready for JSONL serialization.
///
/// Produced from a `Message` and caller-supplied endpoint name. The `ts` and
/// `content` fields are drawn directly from the source `Message`; `role` and
/// `endpoint` are flattened to owned strings for serialization independence.
#[derive(Debug, serde::Serialize)]
pub struct LogEntry {
    /// Millisecond-precision creation timestamp of the original message.
    pub ts: TimestampMs,
    /// Lowercase role label: "user", "assistant", "system", or "tool".
    pub role: RoleLabel,
    /// Endpoint name the message was sent to or received from.
    pub endpoint: EndpointName,
    /// Full text content of the message.
    pub content: LogContent,
}

/// Serialize a `LogEntry` to a single compact JSON line with no trailing newline.
///
/// Used by the actor to construct each line before appending to the JSONL file.
/// Content with embedded newlines is JSON-escaped so the result is always a
/// single line safe for `\n`-delimited JSONL.
pub(crate) fn format_as_jsonl(entry: &LogEntry) -> OutputText {
    OutputText::from(
        serde_json::to_string(entry)
            .unwrap_or_else(|e| format!(r#"{{"error":"serialization failed: {e}"}}"#)),
    )
}

/// Map a `Role` variant to its canonical lowercase log label.
///
/// Called when building a `LogEntry` from a `Message`. The returned label is
/// stored in `LogEntry::role` and written verbatim to the JSONL file.
fn role_label(role: &Role) -> RoleLabel {
    match role {
        Role::User => RoleLabel::new("user"),
        Role::Assistant => RoleLabel::new("assistant"),
        Role::System => RoleLabel::new("system"),
        Role::Tool => RoleLabel::new("tool"),
    }
}

/// Convert a `Message` to a `LogEntry` using the given endpoint name.
///
/// Extracts `ts` and `content` from the message; derives `role` via `role_label`.
/// Called once per message inside the actor before appending to the file.
pub fn message_to_entry(msg: &Message, endpoint: &EndpointName) -> LogEntry {
    LogEntry {
        ts: msg.timestamp,
        role: role_label(&msg.role),
        endpoint: endpoint.clone(),
        content: LogContent::new(msg.content.clone().into_inner()),
    }
}

/// Convert a `HistoryFeedMessage` to a `LogEntry` using the given endpoint name.
///
/// Delegates to `message_to_entry` for both `UserEntry` and `LlmEntry` variants,
/// preserving the role, timestamp, content, and endpoint from the wrapped `Message`.
/// Called inside the actor to normalize feed messages before appending to the JSONL file.
pub fn history_entry_to_log_entry(entry: &HistoryFeedMessage, endpoint: &EndpointName) -> LogEntry {
    match entry {
        HistoryFeedMessage::UserEntry(msg) => message_to_entry(msg, endpoint),
        HistoryFeedMessage::LlmEntry(msg) => message_to_entry(msg, endpoint),
    }
}

/// Build the message-log file name for the current session.
///
/// Returns a filename of the form `<unix_seconds>_msg.jsonl`. The timestamp is
/// captured once at session start (by the actor on spawn) so all messages
/// within a TUI session share the same file.
pub fn message_log_file_name(session_start_secs: TimestampSecs) -> PathBuf {
    PathBuf::from(format!("{session_start_secs}_msg.jsonl"))
}

/// Build the tracing-log file name for the current session.
///
/// Returns a filename of the form `<unix_seconds>_app.log`.
pub fn app_log_file_name(session_start_secs: TimestampSecs) -> PathBuf {
    PathBuf::from(format!("{session_start_secs}_app.log"))
}

/// Build the TUI-log file name for the current session.
///
/// Returns a filename of the form `<unix_seconds>_tui.log`.
pub fn tui_log_file_name(session_start_secs: TimestampSecs) -> PathBuf {
    PathBuf::from(format!("{session_start_secs}_tui.log"))
}

/// Build the LLM-raw-request log file name for the current session.
///
/// Returns a filename of the form `<unix_seconds>_llm.jsonl`. Each line is a
/// JSON object capturing one outgoing request body (direction, provider, model,
/// and the serialized JSON payload). Structured as JSONL so the file can be
/// tail-followed or parsed line-by-line for debugging.
pub fn llm_log_file_name(session_start_secs: TimestampSecs) -> PathBuf {
    PathBuf::from(format!("{session_start_secs}_llm.jsonl"))
}

///
/// Used by the actor at spawn time to derive the log file name for this session.
pub fn current_unix_secs() -> TimestampSecs {
    TimestampSecs::new(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}
