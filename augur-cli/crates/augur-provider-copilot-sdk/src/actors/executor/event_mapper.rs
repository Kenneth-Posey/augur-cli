//! Pure mapping from local `SessionEvent` values to `AgentOutput` values.
//!
//! This module contains no I/O and no SDK types. The actor translates
//! SDK-specific events into `SessionEvent` before calling `map_session_event`,
//! so these functions are fully testable without the `copilot-executor` feature.

use super::commands::SessionEvent;
use augur_domain::plan_tree::NodeStatus;
use augur_domain::string_newtypes::{FailureReason, OutputText, StringNewtype};
use augur_domain::types::AgentOutput;

const STATUS_IN_PROGRESS: &str = "in_progress";
const STATUS_DONE: &str = "done";
const STATUS_FAILED: &str = "failed";

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
        SessionEvent::PlanNodeUpdated {
            node_id,
            status,
            notes,
        } => {
            let node_status = parse_node_status(status, notes.as_deref());
            Some(AgentOutput::PlanNodeUpdate {
                node_id: node_id.clone(),
                status: node_status,
                notes: notes.as_deref().map(OutputText::new),
            })
        }
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

/// Parse a status string from the `update_plan_step` tool into a `NodeStatus`.
///
/// Expected values: `"in_progress"`, `"done"`, `"failed"`. Any unrecognised
/// string maps to `Pending` as a safe default. `notes` is used as the failure
/// message when status is `"failed"`.
fn parse_node_status(status: &str, notes: Option<&str>) -> NodeStatus {
    match status {
        STATUS_IN_PROGRESS => NodeStatus::InProgress,
        STATUS_DONE => NodeStatus::Done,
        STATUS_FAILED => NodeStatus::Failed(FailureReason::new(notes.unwrap_or(""))),
        _ => NodeStatus::Pending,
    }
}
