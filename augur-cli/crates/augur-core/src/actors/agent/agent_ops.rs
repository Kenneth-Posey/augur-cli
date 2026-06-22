//! Pure types and helpers for the agent actor. No I/O, no async.

use augur_domain::config::types::AppConfig;
use augur_domain::domain::newtypes::{Count, TokenCount};
use augur_domain::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use augur_domain::domain::types::{Message, ToolCall};
use augur_domain::domain::{ToolCallResult, ToolDefinition};

pub use augur_domain::domain::types::AgentOutput;
// Re-export persistence functions for callers
pub use super::persistence_ops::{
    build_message_records, make_error_annotation, merge_with_error_annotations,
};

/// Maximum tool-call re-entry loops before the agent stops with an error.
///
/// Prevents infinite tool-call cycles when the LLM keeps returning tool calls.
/// Used as the default value for `TurnConfig::max_iterations`. The agent sends
/// `AgentOutput::Error` and halts the turn when this limit is reached.
pub const DEFAULT_MAX_ITERATIONS: Count = Count::of(1000);

/// Default max context length in tokens when no per-model configuration is available.
///
/// Used as a safe fallback when the provider catalog does not specify a
/// `max_context_length` for the current model. Set to 200K as a conservative
/// value that fits within most common provider context windows (200K-1M tokens)
/// while leaving headroom for system prompt, tool definitions, and the 80%
/// guard threshold (`max_context_length * 0.8`).
pub const DEFAULT_MAX_CONTEXT_LENGTH: TokenCount = TokenCount::of(200_000);

/// Per-turn configuration passed to `process_turn`.
pub struct TurnConfig {
    /// Maximum tool-call re-entry loops before the agent forces a stop.
    pub max_iterations: Count,
    /// The endpoint to use for LLM completion requests this turn.
    pub endpoint: EndpointName,
    /// Optional model override. When set, overrides the endpoint's configured model for this request.
    pub model_override: Option<augur_domain::domain::string_newtypes::ModelId>,
    /// Application config for resolving endpoint definitions.
    pub app_config: AppConfig,
    /// Maximum context length in tokens for the selected model.
    ///
    /// Used to compute the total request-size cap at `max_context_length * 0.8`.
    /// Falls back to [`DEFAULT_MAX_CONTEXT_LENGTH`] when not set by the caller.
    pub max_context_length: TokenCount,
    /// Token threshold that triggers the request-size guard warning.
    ///
    /// When set to a value > 0, the guard warns the LLM when estimated request
    /// tokens exceed this threshold and continues the loop (does not halt).
    /// When zero, falls back to `request_cap_for_context(max_context_length)`.
    /// Typically sourced from the model's `auto_compact_threshold` in the
    /// provider catalog (e.g. 300K for deepseek/deepseek-v4-flash).
    pub request_cap_threshold: TokenCount,
}

/// Accumulated result from consuming one LLM response stream.
///
/// Produced by `consume_stream` at the end of each stream. `text` contains
/// all token content concatenated; `tool_call` holds the first tool call found,
/// or `None` if the LLM produced a plain text response.
pub struct StreamResult {
    /// All token text accumulated from this LLM turn.
    pub text: OutputText,
    /// First tool call found in this turn, if any. Additional tool calls are ignored.
    pub tool_call: Option<ToolCall>,
}

/// Construct a `StreamResult` from accumulated text and an optional tool call.
///
/// Called at the end of `consume_stream` to package the two outputs into a
/// single value. Pure - no side effects. `accumulated_text` is the joined
/// token buffer; `tool_call` passes through unchanged.
pub fn merge_chunks_into_result(
    accumulated_text: &OutputText,
    tool_call: Option<ToolCall>,
) -> StreamResult {
    StreamResult {
        text: accumulated_text.clone(),
        tool_call,
    }
}

/// Convert a tool call and its result into a `Message` for the conversation history.
///
/// Delegates to `Message::tool_result`, which is the single formatting site
/// for tool result messages. Passes `call.id` so the OpenAI-compatible provider
/// can emit `"tool_call_id"` on the tool result message. Called by
/// `process_turn` after every successful tool execution before looping back
/// to the LLM. The result content can be terminal tool output or an immediate
/// dispatch acknowledgement text (for async `task` execution paths).
pub fn tool_result_message(call: &ToolCall, result: &ToolCallResult) -> Message {
    Message::tool_result(call.id.clone(), &call.name, result.output.clone())
}

/// Extend the base system prompt with a formatted list of registered tools.
///
/// Appends a "## Available tools" section that lists each tool's name and
/// description. This ensures the LLM knows what function-call tools are
/// registered and can describe them accurately when asked, rather than running
/// shell commands to discover system-level tools.
///
/// Returns the base prompt unchanged when `tools` is empty, avoiding a
/// dangling empty section in the system context.
///
/// Called once at agent startup in `run()` when constructing `ConversationHistory`.
pub fn build_extended_system_prompt(base: &OutputText, tools: &[ToolDefinition]) -> OutputText {
    if tools.is_empty() {
        return base.clone();
    }
    let tool_lines: String = tools
        .iter()
        .map(|t| format!("- **{}**: {}", t.name.as_str(), t.description))
        .collect::<Vec<_>>()
        .join("\n");
    let size_check_guidance = size_check_guidance_block(tools);
    OutputText::new(format!(
        "{}\n\n## Available tools\nYou have the following function-call tools registered. \
When asked which tools are available, describe these - do not run shell commands to probe the system.{}\n\n{}",
        base.as_str(),
        size_check_guidance,
        tool_lines
    ))
}

fn size_check_guidance_block(tools: &[ToolDefinition]) -> String {
    let has_size_check = tools.iter().any(|tool| tool.name.as_str() == "size_check");
    if !has_size_check {
        return String::new();
    }
    "\n\nWhen a user asks for potentially large reads/searches/listings, call `size_check` first. \
Follow the recommendation in the response: proceed, filter, paginate, or split."
        .to_owned()
}
