//! Persistence-related transformations: converting messages to records, extracting message types, annotating errors.

use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::domain::types::{LlmUsage, Message, Role};
use augur_domain::domain::TimestampMs;
use augur_domain::persistence::types::{MessageRecord, MessageType};

/// Context bundled for message type derivation.
///
/// Captures the message's position in the conversation history alongside
/// the last assistant message index and current LLM usage. Keeps parameter
/// count for `derive_message_type` within the 3-parameter limit.
#[derive(bon::Builder)]
pub struct MessageContext<'a> {
    /// Zero-indexed position of the message in the conversation history.
    pub idx: usize,
    /// Index of the last assistant message, if one exists.
    pub last_assistant_idx: Option<usize>,
    /// Current LLM usage from the latest completion, if available.
    pub last_usage: &'a Option<LlmUsage>,
}

/// Convert a message slice into persistence records.
///
/// Maps each message to its corresponding record type based on role,
/// position, and usage context. The result is a flat vector ready for
/// persistence. Called by `finalize_turn` to prepare messages for saving.
pub fn build_message_records(
    messages: &[Message],
    last_usage: Option<LlmUsage>,
) -> Vec<MessageRecord> {
    let last_assistant_idx = find_last_assistant_idx(messages);
    messages
        .iter()
        .enumerate()
        .map(|(idx, message)| {
            let message_type = derive_message_type(
                message,
                MessageContext::builder()
                    .idx(idx)
                    .maybe_last_assistant_idx(last_assistant_idx)
                    .last_usage(&last_usage)
                    .build(),
            );
            MessageRecord {
                message_type,
                message: message.clone(),
            }
        })
        .collect()
}

/// Find the index of the last assistant message in a conversation.
///
/// Scans the message list in reverse to locate the most recent
/// assistant-role message. Returns `None` if no assistant messages exist.
fn find_last_assistant_idx(messages: &[Message]) -> Option<usize> {
    messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, message)| message.role == Role::Assistant)
        .map(|(idx, _)| idx)
}

/// Derive the persistence type of a message based on role and position.
///
/// Categorizes each message for the session log:
/// - User messages → `MessageType::User`
/// - Tool messages → `MessageType::Tool(name)`
/// - System messages → `MessageType::Assistant` (usually system prompt)
/// - Assistant messages → `MessageType::LlmResponse(usage)` if it's the last assistant
///   message and we have usage data; otherwise `MessageType::Assistant`
fn derive_message_type(message: &Message, ctx: MessageContext<'_>) -> MessageType {
    match message.role {
        Role::User => MessageType::User,
        Role::Tool => MessageType::Tool(parse_tool_name(message.content.as_str())),
        Role::System => MessageType::Assistant,
        Role::Assistant => assistant_message_type(ctx),
    }
}

fn assistant_message_type(ctx: MessageContext<'_>) -> MessageType {
    if ctx.last_assistant_idx == Some(ctx.idx)
        && let Some(usage) = ctx.last_usage
    {
        return MessageType::LlmResponse(usage.clone());
    }
    MessageType::Assistant
}

/// Extract the tool name from a tool message's content.
///
/// Tool message content typically has the format `[tool_name]<output>`.
/// This function strips the brackets to extract just the tool name,
/// or returns `"unknown"` if the format is unexpected.
fn parse_tool_name(content: &str) -> ToolName {
    content
        .strip_prefix('[')
        .and_then(|trimmed| trimmed.find(']').map(|end| &trimmed[..end]))
        .map(ToolName::new)
        .unwrap_or_else(|| ToolName::new("unknown"))
}

/// Build a persistence-only annotation record for a turn-ending error.
///
/// Creates a system-role message marked as an error for display/logging
/// without sending it to the LLM. Used after `process_turn` encounters
/// an error to create a timestamped record of the failure.
pub fn make_error_annotation(error: OutputText) -> MessageRecord {
    MessageRecord {
        message_type: MessageType::Error,
        message: Message {
            role: Role::System,
            content: error,
            timestamp: TimestampMs::now(),
            tool_call_id: None,
            tool_calls: None,
        },
    }
}

/// Merge display-only error annotations into persisted message records.
///
/// Inserts error annotations at the positions they occurred (identified
/// by their count-indexed position) into the base records list, preserving
/// the chronological order. Annotations after all base records are appended
/// at the end. Returns the merged list ready for persistence.
pub fn merge_with_error_annotations(
    base: Vec<MessageRecord>,
    annotations: &[(Count, MessageRecord)],
) -> Vec<MessageRecord> {
    if annotations.is_empty() {
        return base;
    }
    let mut result = Vec::with_capacity(base.len() + annotations.len());
    let mut annotation_idx = 0;
    for (idx, record) in base.into_iter().enumerate() {
        result.push(record);
        annotation_idx = append_annotations_for_position(
            annotations,
            annotation_idx,
            AnnotationInsertTarget {
                position: idx + 1,
                result: &mut result,
            },
        );
    }
    append_remaining_annotations(annotations, annotation_idx, &mut result);
    result
}

struct AnnotationInsertTarget<'a> {
    position: usize,
    result: &'a mut Vec<MessageRecord>,
}

fn append_annotations_for_position(
    annotations: &[(Count, MessageRecord)],
    mut annotation_idx: usize,
    target: AnnotationInsertTarget<'_>,
) -> usize {
    while annotation_idx < annotations.len()
        && annotations[annotation_idx].0.inner() == target.position
    {
        target.result.push(annotations[annotation_idx].1.clone());
        annotation_idx += 1;
    }
    annotation_idx
}

fn append_remaining_annotations(
    annotations: &[(Count, MessageRecord)],
    start: usize,
    result: &mut Vec<MessageRecord>,
) {
    for (_, record) in &annotations[start..] {
        result.push(record.clone());
    }
}
