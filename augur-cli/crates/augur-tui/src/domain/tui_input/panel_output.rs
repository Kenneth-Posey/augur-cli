//! Secondary-panel output helpers for TUI input handling.

use super::*;

/// Apply ask-panel agent output to the secondary ask pane.
pub fn apply_ask_output(state: &mut AppState, output: AgentOutput) {
    tracing::info!(
        output = ?output,
        has_panel = state.interaction.panel.ask_panel.is_some(),
        "tui.panel.ask.apply_output"
    );
    let Some(panel) = state.interaction.panel.ask_panel.as_mut() else {
        return;
    };
    let output = match output {
        AgentOutput::Token(token) => {
            append_panel_token(panel, token);
            return;
        }
        AgentOutput::MessageBreak => {
            panel
                .output
                .push(crate::domain::tui_state::OutputLine::plain(""));
            return;
        }
        AgentOutput::Done | AgentOutput::TurnComplete => {
            panel.thinking = false.into();
            panel
                .output
                .push(crate::domain::tui_state::OutputLine::plain(""));
            panel
                .output
                .push(crate::domain::tui_state::OutputLine::plain(""));
            return;
        }
        output => output,
    };
    apply_ask_secondary_output(panel, output);
}

fn apply_ask_secondary_output(
    panel: &mut crate::domain::tui_state::AskPanelState,
    output: AgentOutput,
) {
    match output {
        AgentOutput::Error(error) => {
            panel.thinking = false.into();
            let text = format!("[error] {error}");
            panel
                .output
                .push(crate::domain::tui_state::OutputLine::error(
                    OutputText::new(text),
                ));
        }
        AgentOutput::Interrupted => {
            panel.thinking = false.into();
        }
        _ => {}
    }
}

/// Apply an agent-feed event to the secondary agent-feed panel.
pub fn apply_agent_feed_output(
    state: &mut AppState,
    entry: impl Into<augur_domain::domain::types::FeedEntry>,
) {
    let entry = entry.into();
    tracing::info!(
        feed_id = ?entry.feed_id,
        event = ?entry.output,
        secondary_view = ?state.interaction.panel.secondary_view,
        input_focus = ?state.interaction.panel.input_focus,
        "tui.panel.agent_feed.apply_output"
    );

    ensure_agent_feed_panel_visible(state);
    let feed_index = ensure_agent_feed(state, entry.feed_id.clone());
    let fallback_model = active_model_fallback(state);
    let feed = &mut state.interaction.panel.agent_feed.feeds[feed_index];
    apply_agent_feed_entry(feed, entry.output, fallback_model);
    if state.interaction.panel.agent_feed.selected_feed == Some(feed_index) {
        sync_selected_feed(state, feed_index);
    }
}

fn ensure_agent_feed_panel_visible(state: &mut AppState) {
    use crate::domain::tui_state::{InputFocus, SecondaryView};
    if state.interaction.panel.secondary_view.is_none() {
        state.interaction.panel.secondary_view = Some(SecondaryView::AgentFeed);
        state.interaction.panel.input_focus = InputFocus::Main;
    }
}

fn active_model_fallback(
    state: &AppState,
) -> Option<augur_domain::domain::string_newtypes::ModelLabel> {
    state
        .prompt
        .models
        .active_id
        .as_ref()
        .map(|id| augur_domain::domain::string_newtypes::ModelLabel::new(id.as_str()))
}

fn apply_agent_feed_entry(
    feed: &mut crate::domain::tui_state::AgentFeedTranscript,
    output: AgentFeedOutput,
    fallback_model: Option<augur_domain::domain::string_newtypes::ModelLabel>,
) {
    let output = match output {
        AgentFeedOutput::ToolEventLine(text) => {
            buffer_tool_event(feed, text);
            return;
        }
        AgentFeedOutput::MessageBreak => {
            apply_message_break(feed);
            return;
        }
        AgentFeedOutput::TaskStarted { name, model } => {
            apply_task_started(
                feed,
                TaskStartModelSelection {
                    task_name: name,
                    task_model: model,
                    fallback_model,
                },
            );
            return;
        }
        output => output,
    };
    apply_agent_feed_terminal_entry(feed, output);
}

fn apply_agent_feed_terminal_entry(
    feed: &mut crate::domain::tui_state::AgentFeedTranscript,
    output: AgentFeedOutput,
) {
    if let AgentFeedOutput::StatusLine(text) = output {
        accumulate_status_line(feed, text);
        return;
    }
    if let AgentFeedOutput::TaskCompleted { name } = output {
        apply_task_completed(feed, name);
        return;
    }
    if let AgentFeedOutput::TaskFailed { name, reason } = output {
        apply_task_failed(feed, name, reason);
        return;
    }
    if let AgentFeedOutput::Clear = output {
        apply_feed_clear(feed);
    }
}

fn ensure_agent_feed(state: &mut AppState, feed_id: augur_domain::domain::types::FeedId) -> usize {
    if let Some(index) = state
        .interaction
        .panel
        .agent_feed
        .feeds
        .iter()
        .position(|feed| feed.feed_id == feed_id)
    {
        return index;
    }
    state
        .interaction
        .panel
        .agent_feed
        .feeds
        .push(crate::domain::tui_state::AgentFeedTranscript {
            feed_id,
            ..Default::default()
        });
    let index = state.interaction.panel.agent_feed.feeds.len() - 1;
    if state.interaction.panel.agent_feed.selected_feed.is_none() {
        state.interaction.panel.agent_feed.selected_feed = Some(0);
    }
    index
}

fn sync_selected_feed(state: &mut AppState, feed_index: usize) {
    let (output, scroll, active_task, current_agent_model, buffers) = {
        let Some(feed) = state.interaction.panel.agent_feed.feeds.get(feed_index) else {
            return;
        };
        (
            feed.output.clone(),
            feed.scroll,
            feed.active_task.clone(),
            feed.current_agent_model.clone(),
            feed.buffers.clone(),
        )
    };
    state.interaction.panel.agent_feed.output = output;
    state.interaction.panel.agent_feed.scroll = scroll;
    state.interaction.panel.agent_feed.active_task = active_task;
    state.interaction.panel.agent_feed.current_agent_model = current_agent_model;
    state.interaction.panel.agent_feed.buffers = buffers;
}

fn apply_message_break(feed: &mut crate::domain::tui_state::AgentFeedTranscript) {
    flush_pending_status_message(feed);
    flush_pending_tool_event(feed);
}

struct TaskStartModelSelection {
    task_name: augur_domain::domain::string_newtypes::AgentName,
    task_model: Option<augur_domain::domain::string_newtypes::ModelLabel>,
    fallback_model: Option<augur_domain::domain::string_newtypes::ModelLabel>,
}

fn apply_task_started(
    feed: &mut crate::domain::tui_state::AgentFeedTranscript,
    model_selection: TaskStartModelSelection,
) {
    flush_pending_tool_event(feed);
    flush_pending_status_message(feed);
    feed.active_task = Some(model_selection.task_name.to_string().into());
    // Use the step model if provided; otherwise fall back to the conversation model.
    feed.current_agent_model = model_selection
        .task_model
        .or(model_selection.fallback_model);
}

fn apply_task_completed(
    feed: &mut crate::domain::tui_state::AgentFeedTranscript,
    name: augur_domain::domain::string_newtypes::AgentName,
) {
    use crate::domain::tui_state::{OutputLine, current_timestamp_ms};
    flush_pending_tool_event(feed);
    flush_pending_status_message(feed);
    let mut line = OutputLine::plain(OutputText::new(format!("{name} completed")));
    line.header.timestamp = Some(current_timestamp_ms());
    feed.output.push(line);
    feed.active_task = None;
    feed.current_agent_model = None;
}

fn apply_task_failed(
    feed: &mut crate::domain::tui_state::AgentFeedTranscript,
    name: augur_domain::domain::string_newtypes::AgentName,
    reason: augur_domain::domain::string_newtypes::OutputText,
) {
    use crate::domain::tui_state::{OutputLine, current_timestamp_ms};
    flush_pending_tool_event(feed);
    flush_pending_status_message(feed);
    let mut line = OutputLine::error(OutputText::new(format!("{name} failed: {reason}")));
    line.header.timestamp = Some(current_timestamp_ms());
    feed.output.push(line);
    feed.active_task = None;
    feed.current_agent_model = None;
}

fn apply_feed_clear(feed: &mut crate::domain::tui_state::AgentFeedTranscript) {
    flush_pending_tool_event(feed);
    flush_pending_status_message(feed);
    feed.output.clear();
    feed.scroll = augur_domain::domain::newtypes::ScrollOffset::default();
    feed.active_task = None;
    feed.current_agent_model = None;
    feed.buffers = crate::domain::tui_state::EventBuffers::default();
}

/// Accumulate `StatusLine` text into the single pending status message.
///
/// Creates a new timestamped pending entry on the first chunk; appends subsequent
/// chunks to the same entry. The pending entry stays visible in the panel live
/// (rendered via `secondary_container`) and is committed to `output` only at
/// structural boundaries: `TaskStarted`, `TaskCompleted`, `TaskFailed`, and `Clear`.
fn accumulate_status_line(
    feed: &mut crate::domain::tui_state::AgentFeedTranscript,
    text: augur_domain::domain::string_newtypes::OutputText,
) {
    use crate::domain::tui_state::{OutputLine, current_timestamp_ms};

    if feed.buffers.pending_status_message.is_none() {
        let mut line = OutputLine::plain(text);
        line.header.timestamp = Some(current_timestamp_ms());
        feed.buffers.pending_status_message = Some(line);
    } else if let Some(ref mut line) = feed.buffers.pending_status_message {
        let combined = format!("{}{}", line.text.as_str(), text.as_str());
        line.text = augur_domain::domain::string_newtypes::OutputText::new(combined);
    }
}

/// Buffer a `ToolEventLine` event to prevent interleaving with streamed messages.
///
/// When a `ToolEventLine` event arrives, it is buffered instead of being
/// immediately pushed to output. This preserves the ordering when tool events
/// arrive between status line chunks. Only one tool event is buffered at a time;
/// if a new tool event arrives before the buffer is flushed, it replaces the
/// previous one.
fn buffer_tool_event(
    feed: &mut crate::domain::tui_state::AgentFeedTranscript,
    text: augur_domain::domain::string_newtypes::OutputText,
) {
    use crate::domain::tui_state::{OutputLine, current_timestamp_ms};

    let mut line = OutputLine::tool_call(text);
    line.header.timestamp = Some(current_timestamp_ms());
    feed.buffers.pending_tool_event = Some(line);
}

/// Flush the pending status message buffer to output.
///
/// If `pending_status_message` is `Some`, moves it to output and clears the buffer.
/// When the buffered text contains `\n`, each segment is pushed as a separate `OutputLine`:
/// the first segment inherits the original header (timestamp), and subsequent segments
/// are plain lines with no timestamp. No-op when the buffer is empty.
fn flush_pending_status_message(feed: &mut crate::domain::tui_state::AgentFeedTranscript) {
    use crate::domain::tui_state::OutputLine;
    use augur_domain::domain::string_newtypes::OutputText;

    let Some(line) = feed.buffers.pending_status_message.take() else {
        return;
    };
    let text = line.text.as_str().to_owned();
    if !text.contains('\n') {
        feed.output.push(line);
        return;
    }
    // Split by newline: first part inherits the original header (timestamp);
    // subsequent parts are plain lines with no header or timestamp.
    for (idx, part) in text.split('\n').enumerate() {
        if idx == 0 {
            let mut first = OutputLine::plain(OutputText::new(part.to_owned()));
            first.header = line.header.clone();
            feed.output.push(first);
        } else {
            feed.output
                .push(OutputLine::plain(OutputText::new(part.to_owned())));
        }
    }
}

/// Flush the pending tool event buffer to output.
///
/// If `pending_tool_event` is Some, moves it to output and clears buffer.
/// No-op if buffer is empty.
fn flush_pending_tool_event(feed: &mut crate::domain::tui_state::AgentFeedTranscript) {
    use crate::domain::tui_state::OutputLine;
    use crate::domain::tui_state::current_timestamp_ms;
    use augur_domain::domain::string_newtypes::OutputText;

    let Some(line) = feed.buffers.pending_tool_event.take() else {
        return;
    };
    let text = line.text.as_str().to_owned();
    if !text.contains('\n') {
        let mut single = line;
        single.header.timestamp = Some(current_timestamp_ms());
        feed.output.push(single);
        return;
    }
    for (idx, part) in text.split('\n').enumerate() {
        if idx == 0 {
            let mut first = OutputLine::tool_call(OutputText::new(part.to_owned()));
            first.header = line.header.clone();
            first.header.timestamp = Some(current_timestamp_ms());
            feed.output.push(first);
        } else {
            feed.output
                .push(OutputLine::tool_call(OutputText::new(part.to_owned())));
        }
    }
}

fn append_panel_token(panel: &mut crate::domain::tui_state::AskPanelState, token: OutputText) {
    let text = token.as_str().to_owned();
    if !text.contains('\n') {
        append_panel_text(panel, &text);
        return;
    }
    for (idx, part) in text.split('\n').enumerate() {
        if idx == 0 {
            append_panel_text(panel, part);
        } else {
            panel
                .output
                .push(crate::domain::tui_state::OutputLine::plain(
                    OutputText::new(part.to_owned()),
                ));
        }
    }
}

fn append_panel_text(panel: &mut crate::domain::tui_state::AskPanelState, text: &str) {
    if let Some(last) = panel.output.last_mut() {
        let combined = format!("{}{}", last.text.as_str(), text);
        last.text = OutputText::new(combined);
    } else {
        panel
            .output
            .push(crate::domain::tui_state::OutputLine::plain(
                OutputText::new(text.to_owned()),
            ));
    }
}
