//! Pure mapping from local `SessionEvent` values to `AgentOutput` values.
//!
//! This module contains no I/O and no SDK types. The actor translates
//! SDK-specific events into `SessionEvent` before calling `map_session_event`,
//! so these functions are fully testable without the `copilot-executor` feature.

use super::commands::SessionEvent;
use augur_domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::types::AgentOutput;

/// Map a local `SessionEvent` to an `AgentOutput`, if one applies.
///
/// Returns `Some(output)` for events that have a direct representation in the
/// agent output stream. Returns `None` for events that are informational only
/// (e.g., `ToolExecutionComplete`, `Unknown`).
///
/// Called by the executor actor's event dispatch loop for every event received
/// from the CLI session. The result is forwarded to the broadcast output channel
/// when `Some`.
pub fn map_session_event(event: &SessionEvent) -> Option<AgentOutput> {
    match event {
        SessionEvent::SessionError { message } => {
            Some(AgentOutput::Error(OutputText::new(message.clone())))
        }
        SessionEvent::SessionIdle => Some(AgentOutput::TurnComplete),
        _ => map_assistant_event(event).or_else(|| map_tool_event(event)),
    }
}

fn map_assistant_event(event: &SessionEvent) -> Option<AgentOutput> {
    if let SessionEvent::AssistantMessageDelta { content } = event {
        return Some(AgentOutput::Token(content.clone()));
    }
    if let SessionEvent::AssistantMessageComplete = event {
        return Some(AgentOutput::Done);
    }
    if let SessionEvent::AssistantUsage { .. } = event {
        return Some(AgentOutput::UsageUpdate { model: None });
    }
    if let SessionEvent::AssistantIntent { intent } = event {
        return Some(AgentOutput::IntentMessage(intent.clone()));
    }
    None
}

fn map_tool_event(event: &SessionEvent) -> Option<AgentOutput> {
    if let SessionEvent::ToolExecutionStart { tool_name, args } = event {
        return Some(AgentOutput::ToolCallStarted {
            name: tool_name.clone(),
            args: args.clone(),
        });
    }
    if let SessionEvent::ToolProgress {
        tool_call_id,
        message,
    } = event
    {
        return Some(AgentOutput::ToolProgress {
            tool_call_id: tool_call_id.clone(),
            message: message.clone(),
        });
    }
    if let SessionEvent::ToolPartialResult {
        tool_call_id,
        output,
    } = event
    {
        return Some(AgentOutput::ToolPartialResult {
            tool_call_id: tool_call_id.clone(),
            output: output.clone(),
        });
    }
    None
}
