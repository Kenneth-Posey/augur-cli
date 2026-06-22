//! Prompt-editing helpers for TUI input handling.

use super::prompt_completion::{
    apply_completion_down, apply_completion_up, apply_history_down, apply_history_up,
    apply_tab_completion,
};
use super::*;
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::PromptText;
use std::ops::ControlFlow;

/// Apply a key action to the session picker state.
pub fn apply_picker_key(state: &mut PickerState, action: &PickerKeyAction) {
    match action {
        PickerKeyAction::SelectUp => {
            state.selected = Count::of(state.selected.inner().saturating_sub(1));
        }
        PickerKeyAction::SelectDown => {
            let max = state.sessions.len().saturating_sub(1);
            state.selected = Count::of(state.selected.inner().saturating_add(1).min(max));
        }
        _ => {}
    }
}

/// Apply a chat-mode key action to the main TUI state.
pub fn apply_key(state: &mut AppState, action: KeyAction) -> ControlFlow<()> {
    match action {
        KeyAction::Quit => return ControlFlow::Break(()),
        KeyAction::Paste(text) => apply_paste(state, text),
        other => apply_non_paste_key(state, &other),
    }
    ControlFlow::Continue(())
}

fn apply_non_paste_key(state: &mut AppState, action: &KeyAction) {
    let _ = apply_prompt_buffer_edit(state, action)
        || apply_prompt_navigation(state, action)
        || apply_completion_navigation(state, action);
}

/// Insert pasted text into the prompt, normalizing embedded newlines to spaces.
pub fn insert_paste(pane: &mut crate::domain::tui_state::PromptPane, text: PromptText) {
    let normalized = text
        .as_str()
        .replace("\r\n", " ")
        .replace(['\r', '\n'], " ");
    pane.buffer.insert_str(pane.cursor, &normalized);
    pane.cursor += normalized.len();
}

fn apply_prompt_buffer_edit(state: &mut AppState, action: &KeyAction) -> bool {
    match action {
        KeyAction::AppendChar(c) => {
            state.prompt.history.pos = None;
            state.prompt.history.draft = None;
            state.prompt.buffer.insert(state.prompt.cursor, *c);
            state.prompt.cursor += c.len_utf8();
        }
        KeyAction::Backspace => backspace_prompt(&mut state.prompt),
        KeyAction::Delete => delete_prompt_char(&mut state.prompt),
        _ => return false,
    }
    true
}

fn apply_prompt_navigation(state: &mut AppState, action: &KeyAction) -> bool {
    if apply_cursor_navigation(&mut state.prompt, action) {
        return true;
    }
    if let Some(scroll) = scroll_delta(action) {
        if scroll < 0 {
            state.scroll_up(Count::of(scroll.unsigned_abs()));
        } else {
            state.scroll_down(Count::of(scroll as usize));
        }
        return true;
    }
    false
}

fn apply_cursor_navigation(
    pane: &mut crate::domain::tui_state::PromptPane,
    action: &KeyAction,
) -> bool {
    if matches!(action, KeyAction::CursorLeft) {
        move_cursor_left(pane);
        return true;
    }
    if matches!(action, KeyAction::CursorRight) {
        move_cursor_right(pane);
        return true;
    }
    if matches!(action, KeyAction::CursorHome) {
        pane.cursor = 0;
        return true;
    }
    if matches!(action, KeyAction::CursorEnd) {
        pane.cursor = pane.buffer.len();
        return true;
    }
    false
}

fn scroll_delta(action: &KeyAction) -> Option<isize> {
    match action {
        KeyAction::ScrollUp(n) => Some(-(*n as isize)),
        KeyAction::ScrollDown(n) => Some(*n as isize),
        _ => None,
    }
}

fn apply_completion_navigation(state: &mut AppState, action: &KeyAction) -> bool {
    match action {
        KeyAction::Tab => apply_tab_completion(state),
        KeyAction::CompletionUp => apply_completion_up_or_history(state),
        KeyAction::CompletionDown => apply_completion_down_or_history(state),
        _ => return false,
    }
    true
}

fn apply_completion_up_or_history(state: &mut AppState) {
    if completions_are_open(&state.prompt.completions) {
        apply_completion_up(state);
        return;
    }
    apply_history_up(state);
}

fn apply_completion_down_or_history(state: &mut AppState) {
    if completions_are_open(&state.prompt.completions) {
        apply_completion_down(state);
        return;
    }
    if state.prompt.history.pos.is_some() {
        apply_history_down(state);
    }
}

fn apply_paste(state: &mut AppState, text: String) {
    state.prompt.history.pos = None;
    state.prompt.history.draft = None;
    insert_paste(&mut state.prompt, PromptText::new(text));
}

fn backspace_prompt(pane: &mut crate::domain::tui_state::PromptPane) {
    if pane.cursor > 0 {
        let new_cursor = prev_char_boundary(&pane.buffer, pane.cursor);
        pane.buffer.drain(new_cursor..pane.cursor);
        pane.cursor = new_cursor;
    }
}

fn delete_prompt_char(pane: &mut crate::domain::tui_state::PromptPane) {
    let buf_len = pane.buffer.len();
    if pane.cursor < buf_len {
        let end = next_char_boundary(&pane.buffer, pane.cursor);
        pane.buffer.drain(pane.cursor..end);
    }
}

fn move_cursor_left(pane: &mut crate::domain::tui_state::PromptPane) {
    if pane.cursor > 0 {
        pane.cursor = prev_char_boundary(&pane.buffer, pane.cursor);
    }
}

fn move_cursor_right(pane: &mut crate::domain::tui_state::PromptPane) {
    if pane.cursor < pane.buffer.len() {
        pane.cursor = next_char_boundary(&pane.buffer, pane.cursor);
    }
}

fn prev_char_boundary(s: &str, byte_pos: usize) -> usize {
    let mut pos = byte_pos.saturating_sub(1);
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn next_char_boundary(s: &str, byte_pos: usize) -> usize {
    let mut pos = byte_pos + 1;
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos.min(s.len())
}

fn completions_are_open(completions: &crate::domain::tui_state::PromptCompletions) -> bool {
    !completions.commands.is_empty()
        || !completions.files.is_empty()
        || !completions.model_picker.items.is_empty()
        || completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .is_some()
}
