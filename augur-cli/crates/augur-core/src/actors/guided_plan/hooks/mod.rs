//! Guided plan hook runners.

/// Subprocess hook runner for shell-command post-phase hooks.
pub mod subprocess;

pub use augur_domain::{
    unavailable_copilot_hook_runner, CopilotAgentHookArgs, CopilotAgentHookFuture,
    CopilotAgentHookRunner, MAX_HOOK_OUTPUT_LINES,
};
