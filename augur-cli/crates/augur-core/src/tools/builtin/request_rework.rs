//! Built-in `request_rework` verdict tool.
//!
//! Registered in the tool registry only during a Copilot agent hook session.
//! The agent calls this to signal that the reviewed phase needs rework, providing
//! a reason describing what must be fixed.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::string_newtypes::{OutputText, ReworkReason, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use tokio::sync::oneshot;

const TOOL_NAME: &str = "request_rework";

/// Tool that signals a rework request from a Copilot agent hook review session.
///
/// Constructed with a `oneshot::Sender<ReworkReason>` that is consumed on the first
/// call to `execute`. The `reason` argument is sent on the channel so the hook
/// runner can transition the phase to `NeedsRework(reason)`. Registered only
/// within the scope of a `run_copilot_agent_hook` session.
/// Consumers: `hooks::copilot_agent`.
pub struct RequestRework {
    tx: std::sync::Mutex<Option<oneshot::Sender<ReworkReason>>>,
}

impl RequestRework {
    /// Construct a new `RequestRework` tool bound to `tx`.
    ///
    /// When `execute` is called, the extracted `reason` string is sent on `tx`.
    /// The sender is consumed on first call; subsequent calls return `is_error: augur_domain::domain::newtypes::IsPredicate::from(true`.)
    pub fn new(tx: oneshot::Sender<ReworkReason>) -> Self {
        RequestRework {
            tx: std::sync::Mutex::new(Some(tx)),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for RequestRework {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Signal that the current phase needs rework before it can be approved. \
             Provide a reason describing what must be fixed.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "description": "Description of what must be fixed before the phase can be approved."
                    }
                },
                "required": ["reason"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(sent))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let reason = ReworkReason::new(args["reason"].as_str().unwrap_or("no reason provided"));

        let sent = self
            .tx
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
            .map(|tx| tx.send(reason.clone()).is_ok())
            .unwrap_or(false);
        tracing::Span::current().record("sent", sent);

        ToolCallResult::builder()
            .name(ToolName::new(TOOL_NAME))
            .output(OutputText::new("rework requested"))
            .is_error(augur_domain::domain::newtypes::IsPredicate::from(!sent))
            .build()
    }
}
