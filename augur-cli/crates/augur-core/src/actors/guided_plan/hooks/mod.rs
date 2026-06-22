//! Guided plan hook runners.

/// Subprocess hook runner for shell-command post-phase hooks.
pub mod subprocess;

pub use augur_domain::{
    CopilotAgentHookArgs, CopilotAgentHookFuture, CopilotAgentHookRunner, MAX_HOOK_OUTPUT_LINES,
    unavailable_copilot_hook_runner,
};
