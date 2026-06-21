//! History adapter ops: pure command-to-feed-message conversion.
//!
//! `HistoryAdapterCmd` carries user or LLM messages for recording.
//! `to_history_entry` maps each command variant to its corresponding
//! [`HistoryFeedMessage`], returning `None` for the `Shutdown` sentinel.

use augur_domain::domain::feeds::HistoryFeedMessage;
use augur_domain::domain::types::Message;
pub use augur_domain::HistoryAdapterCmd;

// ── to_history_entry ──────────────────────────────────────────────────────────

/// Convert a [`HistoryAdapterCmd`] to an optional [`HistoryFeedMessage`].
///
/// Inputs: `cmd` - a reference to the command to convert.
/// Outputs: `Some(HistoryFeedMessage::UserEntry(msg))` for `RecordUser`,
/// `Some(HistoryFeedMessage::LlmEntry(msg))` for `RecordLlm`,
/// and `None` for `Shutdown`.
/// No side effects; this is a pure function.
pub fn to_history_entry(cmd: &HistoryAdapterCmd) -> Option<HistoryFeedMessage> {
    match cmd {
        HistoryAdapterCmd::RecordUser(msg) => Some(HistoryFeedMessage::UserEntry(msg.clone())),
        HistoryAdapterCmd::RecordLlm(msg) => Some(HistoryFeedMessage::LlmEntry(msg.clone())),
        HistoryAdapterCmd::Shutdown => None,
    }
}
