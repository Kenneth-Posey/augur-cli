//! Pure business logic for `OpenRouterTaskActor`. No I/O, no async.
//!
//! All functions in this module are deterministic and side-effect-free.
//! They mirror the structure of `src/actors/agent/ops.rs`.

use augur_domain::newtypes::Count;
use augur_domain::string_newtypes::{AgentName, OutputText, StringNewtype};
use augur_domain::task_types::{AgentInstructions, AgentSpecName, InstructionPrefix, TaskSignal};
use augur_domain::tool_types::ToolDefinition;
use augur_domain::types::{AgentFeedOutput, Message};

/// Maximum tool-call re-entry loops before the task stops with a failure.
///
/// Prevents infinite tool-call cycles when the LLM keeps returning tool calls.
/// The task sends `TaskSignal::Failed` and halts when this limit is reached.
pub const DEFAULT_MAX_ITERATIONS: Count = Count::of(100);

/// Build the task system prompt from agent instructions and the registered tool list.
///
/// Appends a "## Available tools" section listing each tool's name and description
/// when tools are present. Returns the instructions unchanged when `tools` is empty,
/// avoiding a dangling empty section in the system context.
///
/// # Parameters
///
/// - `instructions`: free-form instruction text from the agent spec.
/// - `tools`: registered tool definitions to surface in the system prompt.
///
/// # Returns
///
/// An `OutputText` containing the instructions followed by the tool list section,
/// or the instructions alone when no tools are registered.
pub fn build_task_system_prompt(
    instructions: &AgentInstructions,
    tools: &[ToolDefinition],
) -> OutputText {
    let base = instructions.as_ref();
    if tools.is_empty() {
        return OutputText::new(base);
    }
    let tool_lines: String = tools
        .iter()
        .map(|t| format!("- **{}**: {}", t.name.as_str(), t.description.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    let size_check_guidance = if tools.iter().any(|tool| tool.name.as_str() == "size_check") {
        "\n\nWhen a request may produce large output, call `size_check` before heavy reads/searches. \
Follow its recommendation (proceed/filter/paginate/split) in your next response."
    } else {
        ""
    };
    OutputText::new(format!(
        "{base}\n\n## Available tools\n\
         You have the following function-call tools registered. \
         When asked which tools are available, describe these - \
         do not run shell commands to probe the system.{size_check_guidance}\n\n{tool_lines}"
    ))
}

/// Prepend instruction prefix messages to a message list.
///
/// Returns a new `Vec<Message>` with the prefix messages first, followed by the
/// original messages. Does not mutate either input. When the prefix is empty the
/// original messages are returned unchanged as a new allocation.
///
/// # Parameters
///
/// - `prefix`: ordered list of messages to inject at the front.
/// - `messages`: existing conversation messages, typically from `history.messages_for_request()`.
///
/// # Returns
///
/// Combined message list: `[prefix_messages..., messages...]`.
pub fn prepend_prefix(prefix: &InstructionPrefix, messages: &[Message]) -> Vec<Message> {
    if prefix.is_empty() {
        return messages.to_vec();
    }
    let mut combined = prefix.0.clone();
    combined.extend_from_slice(messages);
    combined
}

/// Map a `TaskSignal` to the corresponding `AgentFeedOutput` panel event.
///
/// Used at task completion to emit the final status event to the TUI agent feed.
/// `Cancelled` is mapped to `TaskFailed` with reason `"cancelled"`.
///
/// # Parameters
///
/// - `name`: spec name of the task that completed or failed.
/// - `signal`: lifecycle outcome to convert.
///
/// # Returns
///
/// `AgentFeedOutput::TaskCompleted` on success, `AgentFeedOutput::TaskFailed` on
/// failure or cancellation.
pub fn signal_to_feed_event(name: &AgentSpecName, signal: &TaskSignal) -> AgentFeedOutput {
    let agent_name = AgentName::new(name.as_ref());
    match signal {
        TaskSignal::Completed { .. } => AgentFeedOutput::TaskCompleted { name: agent_name },
        TaskSignal::Failed { reason } => AgentFeedOutput::TaskFailed {
            name: agent_name,
            reason: reason.clone(),
        },
        TaskSignal::Cancelled => AgentFeedOutput::TaskFailed {
            name: agent_name,
            reason: OutputText::new("cancelled"),
        },
    }
}

/// Determine whether the iteration limit has been reached.
///
/// Returns `true` when `iterations >= max`, signalling the task loop to stop.
/// Called at the top of each tool-call re-entry iteration.
///
/// # Parameters
///
/// - `iterations`: the number of iterations completed so far.
/// - `max`: the configured maximum number of iterations.
///
/// # Returns
///
/// `true` when the limit is reached or exceeded; `false` otherwise.
pub fn is_at_iteration_limit(iterations: Count, max: Count) -> augur_domain::newtypes::IsPredicate {
    augur_domain::newtypes::IsPredicate::from(iterations >= max)
}
