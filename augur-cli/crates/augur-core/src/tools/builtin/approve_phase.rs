//! Built-in `approve_phase` verdict tool.
//!
//! Registered in the tool registry only during a Copilot agent hook session.
//! The agent calls this to signal that the reviewed phase is complete and approved.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use tokio::sync::oneshot;

const TOOL_NAME: &str = "approve_phase";

/// Tool that signals phase approval from a Copilot agent hook review session.
///
/// Constructed with a `oneshot::Sender<bool>` that is consumed on the first
/// call to `execute`. Subsequent calls (if any) return `is_error: augur_domain::domain::newtypes::IsPredicate::from(true` because)
/// the sender has already been consumed. Registered only within the scope of a
/// `run_copilot_agent_hook` session. Consumers: `hooks::copilot_agent`.
pub struct ApprovePhase {
    tx: std::sync::Mutex<Option<oneshot::Sender<bool>>>,
}

impl ApprovePhase {
    /// Construct a new `ApprovePhase` tool bound to `tx`.
    ///
    /// When `execute` is called, `true` is sent on `tx` to signal approval
    /// to the hook runner. The sender is consumed on first call.
    #[cfg_attr(not(test), allow(dead_code))]
    fn new(tx: oneshot::Sender<bool>) -> Self {
        ApprovePhase {
            tx: std::sync::Mutex::new(Some(tx)),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for ApprovePhase {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Signal that the current phase is complete and approved. \
             Call this when the review finds no issues.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        )
    }

    #[tracing::instrument(skip(self, _args))]
    async fn execute(&self, _args: serde_json::Value) -> ToolCallResult {
        let sent = self
            .tx
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
            .map(|tx| tx.send(true).is_ok())
            .unwrap_or(false);
        ToolCallResult::builder()
            .name(ToolName::new(TOOL_NAME))
            .output(OutputText::new("approved"))
            .is_error(augur_domain::domain::newtypes::IsPredicate::from(!sent))
            .build()
    }
}

#[cfg(test)]
#[path = "../../../tests/tools/builtin/approve_phase.tests.rs"]
mod tests;
