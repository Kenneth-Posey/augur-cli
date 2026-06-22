//! Session restore helpers: hydrate output from a loaded session record.

use crate::actors::tui::tui_actor::TuiHandles;
use crate::domain::tui_state::{AppScreen, AppState, ConversationMode, OutputLine};
use augur_domain::domain::newtypes::TimestampMs;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::domain::tool_call_formatting::format_tool_call_line;
use augur_domain::domain::types::Message;
use augur_domain::domain::types::ToolCall;
use augur_domain::persistence::types::{MessageType, SessionRecord};

/// Apply a successfully loaded `SessionRecord` to all live actors and UI state.
///
/// Hydrates the output pane with user, assistant, and system messages from the
/// record so the conversation history is immediately visible. Assistant messages
/// that carry `tool_calls` are rendered with restored tool-call rows before the
/// assistant text body. Tool-result messages are skipped to avoid duplicating the
/// raw tool payload in the feed. The system confirmation line is pushed last,
/// after all history lines.
///
/// After restoring persistence and history, calls `handles.agent.replace_session`
/// with the SDK session ID from the record. For Copilot sessions this reconnects
/// the actor to the saved SDK session; for other providers the call is a no-op.
///
/// Consumers: `restore_session` in `actor.rs` on a successful session load.
pub(crate) async fn apply_restored_session(
    state: &mut AppState,
    record: SessionRecord,
    handles: &TuiHandles<'_>,
) {
    let endpoint = record.meta.endpoint_name.clone();
    let count = record.state.messages.len();
    let sdk_session_id = record.meta.flags.sdk_session_id.clone();
    if handles
        .session
        .set_endpoint(endpoint.clone())
        .await
        .is_err()
    {
        state.push_error_line("[error] failed to restore session endpoint");
        state.push_output_newline();
        state.interaction.screen = AppScreen::Conversation;
        state.interaction.mode = ConversationMode::Chat;
        return;
    }
    handles.persistence.restore_from(&record);
    state.status.context_window.reset_for_new_session();
    hydrate_output_from_messages(state, &record);
    state
        .output
        .scroll_offset
        .set(augur_domain::domain::newtypes::ScrollOffset::of(0));
    handles.agent.replace_session(sdk_session_id);
    handles.agent.restore(record.state.messages);
    let msg = format!(
        "[system] restored session (endpoint: {}, {count} messages)",
        endpoint.as_str()
    );
    state.push_system_message(OutputText::new(msg));
    state.push_output_newline();
    state.interaction.screen = AppScreen::Conversation;
    state.interaction.mode = ConversationMode::Chat;
}

/// Emit output lines for each visible message in a restored session record.
///
/// User messages are prefixed with "> " and stamped with the message timestamp.
/// Assistant messages set `pending_response_ts` from the message timestamp so
/// the first line of each response block is stamped on render. Assistant
/// `tool_calls` are restored as `LineKind::ToolCall` rows (with timestamp on the
/// first row of each call) before the assistant text. Tool-result messages carry
/// no user-facing content and are skipped. System messages are preserved as
/// visible transcript boundaries.
///
/// Consumers: `apply_restored_session` in this module.
pub(crate) fn hydrate_output_from_messages(state: &mut AppState, record: &SessionRecord) {
    for msg_record in &record.state.messages {
        hydrate_message_record(state, &msg_record.message_type, &msg_record.message);
    }
}

fn hydrate_message_record(state: &mut AppState, message_type: &MessageType, message: &Message) {
    if is_user_message(message_type) {
        restore_user_message(state, message);
        return;
    }
    if is_assistant_message(message_type) {
        restore_assistant_message(state, message);
        return;
    }
    if is_error_message(message_type) {
        restore_error_message(state, message);
        return;
    }
    if is_system_message(message_type) {
        restore_system_message(state, message);
    }
}

fn is_user_message(message_type: &MessageType) -> bool {
    matches!(message_type, MessageType::User)
}

fn is_assistant_message(message_type: &MessageType) -> bool {
    matches!(
        message_type,
        MessageType::Assistant | MessageType::LlmResponse(_)
    )
}

fn is_error_message(message_type: &MessageType) -> bool {
    matches!(message_type, MessageType::Error)
}

fn is_system_message(message_type: &MessageType) -> bool {
    matches!(message_type, MessageType::System)
}

fn restore_user_message(state: &mut AppState, message: &Message) {
    let line = format!("> {}", message.content.as_str());
    state.push_user_input_line(OutputText::new(line), message.timestamp);
}

fn restore_assistant_message(state: &mut AppState, message: &Message) {
    if let Some(tool_calls) = &message.tool_calls {
        push_restored_tool_calls(state, tool_calls, message.timestamp);
    }
    push_restored_assistant_text(state, message.content.as_str(), message.timestamp);
}

fn restore_error_message(state: &mut AppState, message: &Message) {
    state.push_error_line(format!("[error] {}", message.content.as_str()));
}

fn restore_system_message(state: &mut AppState, message: &Message) {
    state.push_system_message(message.content.as_str());
}

fn push_restored_tool_calls(state: &mut AppState, tool_calls: &[ToolCall], timestamp: TimestampMs) {
    for call in tool_calls {
        push_restored_tool_call(state, call, timestamp);
    }
}

fn push_restored_tool_call(state: &mut AppState, call: &ToolCall, timestamp: TimestampMs) {
    let formatted = format_tool_call_line(call.name.clone(), &call.arguments);
    let mut parts = formatted.as_str().split('\n');
    if let Some(first) = parts.next() {
        let mut first_line = OutputLine::tool_call_with_metadata(
            OutputText::new(first),
            call.name.clone(),
            call.arguments.clone(),
        );
        first_line.header.timestamp = Some(timestamp);
        state.output.lines.push(first_line);
    }
    for part in parts {
        state.output.lines.push(OutputLine::tool_call(part));
    }
}

fn push_restored_assistant_text(state: &mut AppState, text: &str, timestamp: TimestampMs) {
    if text.is_empty() {
        return;
    }
    let mut parts = text.split('\n');
    if let Some(first) = parts.next() {
        state.output.lines.push(
            OutputLine::builder()
                .text(OutputText::new(first))
                .kind(crate::domain::tui_state::LineKind::Plain)
                .header(crate::domain::tui_state::LineHeader {
                    timestamp: Some(timestamp),
                    model_prefix: None,
                })
                .build(),
        );
    }
    for part in parts {
        state.output.lines.push(OutputLine::plain(part));
    }
}
