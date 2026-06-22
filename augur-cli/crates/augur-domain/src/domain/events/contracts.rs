//! Event-to-Output Type Contracts: Specification of output contracts for all 41 events.
//!
//! This module defines the contract between events and their output representations,
//! specifying which output type each event produces and whether output is batched
//! or streamed.
//!
//! Phase 1.3 deliverable: Output type mapping for all 41 events.

use crate::domain::string_newtypes::{EventType, StringNewtype};
use crate::domain::IsPredicate;

/// Output category for an event - the semantic type of output produced.
///
/// These categories describe the general shape and role of output from events,
/// distinct from the events themselves. Used by rendering layers to format output
/// appropriately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputCategory {
    /// Token streaming output (AssistantMessageDelta) - character-by-character text
    Token,
    /// Error message output (SessionError, Abort) - failure notification
    Error,
    /// Turn completion marker (SessionIdle) - session ready for new input
    TurnComplete,
    /// Reasoning output (AssistantReasoning) - internal thinking display
    Reasoning,
    /// Tool execution events (ToolExecution*) - tool call tracking
    ToolExecution,
    /// State change notification (SessionStart, SessionResume, etc.) - state transition
    StateChange,
    /// Metadata output (usage, context info) - informational without user-facing semantics
    Metadata,
}

/// Output contract for an event: maps event to output type and batching strategy.
///
/// Specifies the output category and whether output should be streamed (immediately)
/// or batched (accumulated until flush).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputTypeContract {
    /// Event type this contract applies to
    pub event_type: EventType,
    /// Output category for this event
    pub output_category: OutputCategory,
    /// Whether output should be batched (true) or streamed (false)
    pub is_batched: IsPredicate,
}

/// Complete output type mapping for all 41 events.
///
/// Returns the output contract for a given event type, or None if event is suppressed
/// and produces no output.
pub fn output_contract(event_type: &EventType) -> Option<OutputTypeContract> {
    let category = categorize_output(event_type)?;
    let is_batched = should_batch_output(event_type);

    Some(OutputTypeContract {
        event_type: event_type.clone(),
        output_category: category,
        is_batched: is_batched.into(),
    })
}

/// Determine output category for an event type.
///
/// This function maps event types to output categories, defining the semantic role
/// of output produced by each event.
fn categorize_output(event_type: &EventType) -> Option<OutputCategory> {
    categorize_streaming_output(event_type)
        .or_else(|| categorize_error_output(event_type))
        .or_else(|| categorize_turn_complete_output(event_type))
        .or_else(|| categorize_reasoning_output(event_type))
        .or_else(|| categorize_tool_execution_output(event_type))
        .or_else(|| categorize_state_change_output(event_type))
        .or_else(|| categorize_metadata_output(event_type))
}

fn categorize_streaming_output(event_type: &EventType) -> Option<OutputCategory> {
    let s: &str = event_type.as_str();
    matches!(s, "AssistantMessageDelta").then_some(OutputCategory::Token)
}

fn categorize_error_output(event_type: &EventType) -> Option<OutputCategory> {
    let s: &str = event_type.as_str();
    matches!(s, "SessionError" | "Abort").then_some(OutputCategory::Error)
}

fn categorize_turn_complete_output(event_type: &EventType) -> Option<OutputCategory> {
    let s: &str = event_type.as_str();
    matches!(s, "SessionIdle").then_some(OutputCategory::TurnComplete)
}

fn categorize_reasoning_output(event_type: &EventType) -> Option<OutputCategory> {
    let s: &str = event_type.as_str();
    matches!(s, "AssistantReasoning" | "AssistantReasoningDelta")
        .then_some(OutputCategory::Reasoning)
}

fn categorize_tool_execution_output(event_type: &EventType) -> Option<OutputCategory> {
    let s: &str = event_type.as_str();
    matches!(
        s,
        "ToolExecutionStart"
            | "ToolExecutionComplete"
            | "ToolExecutionProgress"
            | "ToolExecutionPartialResult"
    )
    .then_some(OutputCategory::ToolExecution)
}

fn categorize_state_change_output(event_type: &EventType) -> Option<OutputCategory> {
    let s: &str = event_type.as_str();
    matches!(
        s,
        "SessionStart"
            | "SessionResume"
            | "SessionInfo"
            | "SessionShutdown"
            | "SessionSnapshotRewind"
            | "SessionModelChange"
            | "SessionHandoff"
            | "SessionTruncation"
    )
    .then_some(OutputCategory::StateChange)
}

fn categorize_metadata_output(event_type: &EventType) -> Option<OutputCategory> {
    let s: &str = event_type.as_str();
    matches!(
        s,
        "AssistantUsage"
            | "SessionUsageInfo"
            | "SessionCompactionStart"
            | "SessionCompactionComplete"
            | "AssistantIntent"
            | "CustomAgentStarted"
            | "CustomAgentCompleted"
            | "CustomAgentFailed"
    )
    .then_some(OutputCategory::Metadata)
}

/// Determine if output from an event should be batched or streamed.
///
/// Batched output is accumulated until a flush event (e.g., TurnComplete, timeout).
/// Streamed output appears immediately.
///
/// Returns false (stream immediately) for unknown events.
fn should_batch_output(event_type: &EventType) -> bool {
    matches!(
        event_type.as_str(),
        // Batch these events until flush
        "AssistantUsage"            | // Batch token deltas until TurnComplete
        "SessionCompactionStart"    | // Batch compaction progress updates
        "AssistantReasoning"        | // Batch reasoning until timeout/complete
        "AssistantReasoningDelta" // Batch reasoning deltas
    )
}
