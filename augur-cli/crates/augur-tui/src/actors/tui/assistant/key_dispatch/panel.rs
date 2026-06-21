//! Ask-panel and secondary-view helpers for TUI key dispatch.

use crate::actors::tui::tui_actor::TuiHandles;
use crate::domain::tui_state::{
    AppState, AskPanelState, ConversationMode, InputFocus, LineKind, SecondaryView,
};
use augur_domain::domain::newtypes::TimestampMs;
use augur_domain::domain::string_newtypes::{OutputText, PromptText, StringNewtype};
use augur_domain::domain::types::{Message, MessageRecord, MessageType, Role};

/// Flip `input_focus` between Main and Ask. No-op when the ask panel is closed.
pub(crate) fn toggle_ask_focus(state: &mut AppState) {
    if state.interaction.panel.ask_panel.is_none() {
        return;
    }
    state.interaction.panel.input_focus = match state.interaction.panel.input_focus {
        InputFocus::Main => InputFocus::Ask,
        InputFocus::Ask => InputFocus::Main,
    };
}

/// Transition from Plan mode to Chat on Esc when idle with no completions.
pub(crate) fn dispatch_plan_esc(state: &mut AppState) -> Option<()> {
    let no_completions = state.prompt.completions.commands.is_empty()
        && state.prompt.completions.files.is_empty()
        && state.prompt.completions.model_picker.items.is_empty();
    let not_thinking = !state.agent.thinking.is_active;
    if no_completions && not_thinking {
        state.interaction.mode = ConversationMode::Chat;
        Some(())
    } else {
        None
    }
}

/// Toggle the ask secondary view on ShiftTab.
pub(crate) async fn toggle_ask_view(state: &mut AppState, handles: &TuiHandles<'_>) {
    match &state.interaction.panel.secondary_view {
        Some(SecondaryView::Ask) => {
            state.interaction.panel.secondary_view = None;
            state.interaction.panel.input_focus = InputFocus::Main;
        }
        None | Some(SecondaryView::AgentFeed) => {
            open_ask_in_secondary(state, handles);
        }
    }
}

/// Toggle the agent feed secondary view on Ctrl+T.
pub(crate) fn toggle_agent_feed_view(state: &mut AppState) {
    match &state.interaction.panel.secondary_view {
        None => open_agent_feed_view(state),
        Some(SecondaryView::AgentFeed) => {
            state.interaction.panel.secondary_view = None;
        }
        Some(SecondaryView::Ask) => {
            open_agent_feed_view(state);
            state.interaction.panel.input_focus = InputFocus::Main;
        }
    }
}

fn open_agent_feed_view(state: &mut AppState) {
    state.interaction.panel.secondary_view = Some(SecondaryView::AgentFeed);
    ensure_agent_feed_selected(state);
}

fn ensure_agent_feed_selected(state: &mut AppState) {
    let no_selection = state.interaction.panel.agent_feed.selected_feed.is_none();
    let has_feeds = !state.interaction.panel.agent_feed.feeds.is_empty();
    if no_selection && has_feeds {
        state.interaction.panel.agent_feed.selected_feed = Some(0);
        state.sync_selected_agent_feed();
    }
}

/// Submit the ask panel prompt to the ask-panel agent.
pub(crate) fn handle_ask_submit(state: &mut AppState, handles: &TuiHandles<'_>) {
    let text = state.take_prompt();
    if text.as_str().is_empty() {
        return;
    }
    if let Some(ref mut panel) = state.interaction.panel.ask_panel {
        panel.thinking = true.into();
        panel
            .output
            .push(crate::domain::tui_state::OutputLine::user_input(
                OutputText::new(format!("> {}", text.as_str())),
            ));
        panel
            .output
            .push(crate::domain::tui_state::OutputLine::plain(
                OutputText::new(""),
            ));
    }
    handles.tools.ask.submit(PromptText::new(text.into_inner()));
}

/// Open the ask panel in the secondary view and seed it from the main conversation.
pub(crate) fn open_ask_in_secondary(state: &mut AppState, handles: &TuiHandles<'_>) {
    if state.interaction.panel.ask_panel.is_none() {
        state.interaction.panel.ask_panel = Some(AskPanelState::default());
    }
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Ask;
    seed_ask_context(state, handles);
}

fn seed_ask_context(state: &mut AppState, handles: &TuiHandles<'_>) {
    let snapshot = main_conversation_snapshot(state);
    if let Some(ref mut panel) = state.interaction.panel.ask_panel {
        if panel.seeded.into() {
            return;
        }
        handles.tools.ask.restore(snapshot);
        panel.seeded = true.into();
    }
}

fn main_conversation_snapshot(state: &AppState) -> Vec<MessageRecord> {
    state
        .output
        .lines
        .iter()
        .filter_map(output_line_to_record)
        .collect()
}

fn output_line_to_record(line: &crate::domain::tui_state::OutputLine) -> Option<MessageRecord> {
    let timestamp = line.header.timestamp.unwrap_or_else(TimestampMs::now);
    match line.kind {
        LineKind::UserInput => user_line_record(line, timestamp),
        LineKind::Plain => plain_line_record(line, timestamp),
        LineKind::System => system_line_record(line, timestamp),
        LineKind::ToolCall | LineKind::Error | LineKind::SelfFeedback => None,
    }
}

fn user_line_record(
    line: &crate::domain::tui_state::OutputLine,
    timestamp: TimestampMs,
) -> Option<MessageRecord> {
    if line.text.as_str().is_empty() {
        return None;
    }
    Some(MessageRecord {
        message_type: MessageType::User,
        message: Message {
            role: Role::User,
            content: OutputText::new(line.text.as_str().trim_start_matches("> ")),
            timestamp,
            tool_call_id: None,
            tool_calls: None,
        },
    })
}

fn plain_line_record(
    line: &crate::domain::tui_state::OutputLine,
    timestamp: TimestampMs,
) -> Option<MessageRecord> {
    if line.text.as_str().is_empty() {
        return None;
    }
    let is_system_message = line.text.as_str().starts_with("[system]");
    let role = if is_system_message {
        Role::System
    } else {
        Role::Assistant
    };
    let message_type = if is_system_message {
        MessageType::System
    } else {
        MessageType::Assistant
    };
    Some(MessageRecord {
        message_type,
        message: Message {
            role,
            content: line.text.clone(),
            timestamp,
            tool_call_id: None,
            tool_calls: None,
        },
    })
}

fn system_line_record(
    line: &crate::domain::tui_state::OutputLine,
    timestamp: TimestampMs,
) -> Option<MessageRecord> {
    if line.text.as_str().is_empty() {
        return None;
    }
    Some(MessageRecord {
        message_type: MessageType::System,
        message: Message {
            role: Role::System,
            content: line.text.clone(),
            timestamp,
            tool_call_id: None,
            tool_calls: None,
        },
    })
}
