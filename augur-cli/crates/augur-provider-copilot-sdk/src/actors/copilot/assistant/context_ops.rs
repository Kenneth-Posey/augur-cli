//! SDK error formatting and logging helpers for `CopilotChatActor`.
//!
//! Extracted from `actor.rs` to keep the actor file within the 200-logic-line
//! threshold. Covers SDK error formatting/logging for the command loop error paths.
//!
//! Consumers: `actor::run_command_loop`.

use augur_domain::OutputText;

/// Format a `CopilotError` as a user-facing string.
///
/// For `JsonRpc` errors, includes code, message, and the optional `data`
/// payload so callers can see the full RPC error without relying on the
/// default `Display` impl which drops `data`. All other variants use the
/// standard `Display` output.
/// Consumers: `actor::run_command_loop` `SendMessage` and `Compact` arms.
pub fn format_sdk_error(e: &copilot_sdk::CopilotError) -> OutputText {
    match e {
        copilot_sdk::CopilotError::JsonRpc {
            code,
            message,
            data,
        } => match data {
            Some(d) => OutputText::from(format!("JSON-RPC error {code}: {message} (data: {d})")),
            None => OutputText::from(format!("JSON-RPC error {code}: {message}")),
        },
        other => OutputText::from(other.to_string()),
    }
}

/// Log a `CopilotError` with structured fields at the appropriate level.
///
/// `JsonRpc` errors are logged at `error` level with `code`, `message`, and
/// optional `data` as separate structured fields so log aggregators can filter
/// on them. All other error variants are logged at `warn` level.
/// Consumers: `actor::run_command_loop` `SendMessage` and `Compact` arms.
pub fn log_sdk_error(e: &copilot_sdk::CopilotError, context: &OutputText) {
    match e {
        copilot_sdk::CopilotError::JsonRpc {
            code,
            message,
            data,
        } => {
            tracing::error!(
                rpc_code = code,
                rpc_message = %message,
                rpc_data = ?data,
                "{}", context
            );
        }
        other => {
            tracing::warn!(error = %other, "{}", context);
        }
    }
}
