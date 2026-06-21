//! Tool schema and execution-result domain types.

use crate::domain::newtypes::IsPredicate;
use crate::domain::string_newtypes::{OutputText, ToolDescription, ToolName};

/// Schema describing a tool available to the LLM for function calling.
///
/// The canonical definition of a tool's interface. Passed to LLM API requests
/// in the tools/functions array. `parameters` must be a JSON Schema object node.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    /// Unique tool identifier; must match the name returned in `StreamChunk::ToolCall`.
    pub name: ToolName,
    /// Human-readable description sent to the LLM explaining when to call this tool.
    pub description: ToolDescription,
    /// JSON Schema `"object"` node describing the tool arguments.
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new `ToolDefinition`.
    pub fn new(
        name: impl Into<ToolName>,
        description: impl Into<ToolDescription>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// The result of executing a tool call.
///
/// Returned by every `ToolHandler::execute` implementation. `is_error` signals
/// whether the underlying operation failed; the agent uses this flag to decide
/// whether to surface the error to the user or continue the conversation.
#[derive(Clone, Debug, bon::Builder)]
pub struct ToolCallResult {
    /// Name of the tool that produced this result; mirrors the call request name.
    pub name: ToolName,
    /// Tool output text forwarded to the LLM as a tool-result message.
    pub output: OutputText,
    /// True when the underlying operation failed.
    pub is_error: IsPredicate,
    /// Human-readable summary shown in the TUI before the detailed tool entry.
    pub session_log: Option<OutputText>,
}
