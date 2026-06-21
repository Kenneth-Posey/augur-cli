//! Prompt completion and history helpers for TUI input handling.

use super::*;
/// Apply the currently selected completion candidate to the prompt buffer.
pub(crate) fn apply_tab_completion(state: &mut AppState) {
    let n_cmd = state.prompt.completions.commands.len();
    if n_cmd > 0 {
        let idx = state.prompt.completions.command_selected.unwrap_or(0);
        let cmd = state.prompt.completions.commands[idx.min(n_cmd - 1)];
        let text = completion_text_for(cmd.usage);
        state.prompt.cursor = text.len();
        state.prompt.buffer = text.into();
        state.prompt.completions.commands.clear();
        state.prompt.completions.command_selected = None;
        return;
    }
    let n_file = state.prompt.completions.files.len();
    if n_file > 0 {
        apply_file_completion(state);
        return;
    }
    let n_model = state.prompt.completions.model_picker.items.len();
    if n_model == 0 {
        return;
    }
    let idx = state.prompt.completions.model_picker.selected.unwrap_or(0);
    let id = state.prompt.completions.model_picker.items[idx.min(n_model - 1)]
        .id
        .clone();
    let text = format!("/model {}", id.as_str());
    state.prompt.cursor = text.len();
    state.prompt.buffer = text.into();
    state.prompt.completions.model_picker.items.clear();
    state.prompt.completions.model_picker.selected = None;
}

/// Apply the currently selected file completion to the active `@` token.
pub(crate) fn apply_file_completion(state: &mut AppState) {
    let n = state.prompt.completions.files.len();
    if n == 0 {
        return;
    }
    let idx = state.prompt.completions.file_selected.unwrap_or(0);
    let path = state.prompt.completions.files[idx.min(n - 1)].path.clone();
    replace_file_token_in_buffer(state, &path);
    state.prompt.completions.files.clear();
    state.prompt.completions.file_selected = None;
}

fn replace_file_token_in_buffer(state: &mut AppState, path: &FilePath) {
    let Some(at_pos) = state.prompt.buffer.rfind('@') else {
        return;
    };
    let rest = &state.prompt.buffer[at_pos + 1..];
    let token_end = rest
        .find(char::is_whitespace)
        .map_or(state.prompt.buffer.len(), |rel| at_pos + 1 + rel);
    let replacement = format!("@{}", path.as_str());
    state
        .prompt
        .buffer
        .replace_range(at_pos..token_end, &replacement);
    state.prompt.cursor = at_pos + replacement.len();
}

/// Move the active completion selection downward.
pub(super) fn apply_completion_down(state: &mut AppState) {
    if advance_thinking_mode_selection_down(state) {
        return;
    }
    if advance_command_selection_down(state) || advance_file_selection_down(state) {
        return;
    }
    advance_model_selection_down(state);
}

/// Move the active completion selection upward.
pub(super) fn apply_completion_up(state: &mut AppState) {
    if advance_thinking_mode_selection_up(state) {
        return;
    }
    if advance_command_selection_up(state) || advance_file_selection_up(state) {
        return;
    }
    advance_model_selection_up(state);
}

fn user_input_history(state: &AppState) -> Vec<String> {
    state
        .output
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::UserInput)
        .map(|l| {
            let text = l.text.as_str();
            text.strip_prefix("> ").unwrap_or(text).to_owned()
        })
        .collect()
}

/// Move the prompt history cursor toward older submitted entries.
pub(super) fn apply_history_up(state: &mut AppState) {
    let entries = user_input_history(state);
    let n = entries.len();
    if n == 0 {
        return;
    }
    // Save the live buffer as draft the first time we enter history navigation.
    if state.prompt.history.pos.is_none() {
        state.prompt.history.draft = Some(state.prompt.buffer.to_string());
    }
    let next_pos = next_history_up_position(state.prompt.history.pos, n);
    let entry = entries[n - 1 - next_pos].clone();
    state.prompt.buffer = entry.into();
    state.prompt.cursor = state.prompt.buffer.len();
    state.prompt.history.pos = Some(next_pos);
}

fn next_history_up_position(current: Option<usize>, len: usize) -> usize {
    match current {
        None => 0,
        Some(i) if i + 1 < len => i + 1,
        Some(i) => i,
    }
}

/// Move the prompt history cursor toward newer submitted entries.
pub(super) fn apply_history_down(state: &mut AppState) {
    let entries = user_input_history(state);
    let n = entries.len();
    match state.prompt.history.pos {
        None => {}
        Some(0) => {
            let draft = state.prompt.history.draft.take().unwrap_or_default();
            state.prompt.buffer = draft.into();
            state.prompt.cursor = state.prompt.buffer.len();
            state.prompt.history.pos = None;
        }
        Some(i) => {
            let next_pos = i - 1;
            let entry = entries[n - 1 - next_pos].clone();
            state.prompt.buffer = entry.into();
            state.prompt.cursor = state.prompt.buffer.len();
            state.prompt.history.pos = Some(next_pos);
        }
    }
}

fn advance_thinking_mode_selection_down(state: &mut AppState) -> bool {
    let is_open = state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id
        .is_some();
    if !is_open {
        return false;
    }
    let len = augur_domain::domain::thinking_mode::ReasoningEffort::options().len();
    state.prompt.completions.model_picker.thinking_mode.selected = next_completion_selection(
        state.prompt.completions.model_picker.thinking_mode.selected,
        len,
    );
    true
}

fn advance_thinking_mode_selection_up(state: &mut AppState) -> bool {
    let is_open = state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id
        .is_some();
    if !is_open {
        return false;
    }
    let len = augur_domain::domain::thinking_mode::ReasoningEffort::options().len();
    state.prompt.completions.model_picker.thinking_mode.selected = previous_completion_selection(
        state.prompt.completions.model_picker.thinking_mode.selected,
        len,
    );
    true
}

fn advance_command_selection_down(state: &mut AppState) -> bool {
    let len = state.prompt.completions.commands.len();
    if len == 0 {
        return false;
    }
    state.prompt.completions.command_selected =
        next_completion_selection(state.prompt.completions.command_selected, len);
    true
}

fn advance_file_selection_down(state: &mut AppState) -> bool {
    let len = state.prompt.completions.files.len();
    if len == 0 {
        return false;
    }
    state.prompt.completions.file_selected =
        next_completion_selection(state.prompt.completions.file_selected, len);
    true
}

fn advance_model_selection_down(state: &mut AppState) {
    let len = state.prompt.completions.model_picker.items.len();
    if len == 0 {
        return;
    }
    state.prompt.completions.model_picker.selected =
        next_completion_selection(state.prompt.completions.model_picker.selected, len);
}

fn advance_command_selection_up(state: &mut AppState) -> bool {
    let len = state.prompt.completions.commands.len();
    if len == 0 {
        return false;
    }
    state.prompt.completions.command_selected =
        previous_completion_selection(state.prompt.completions.command_selected, len);
    true
}

fn advance_file_selection_up(state: &mut AppState) -> bool {
    let len = state.prompt.completions.files.len();
    if len == 0 {
        return false;
    }
    state.prompt.completions.file_selected =
        previous_completion_selection(state.prompt.completions.file_selected, len);
    true
}

fn advance_model_selection_up(state: &mut AppState) {
    let len = state.prompt.completions.model_picker.items.len();
    if len == 0 {
        return;
    }
    state.prompt.completions.model_picker.selected =
        previous_completion_selection(state.prompt.completions.model_picker.selected, len);
}

fn next_completion_selection(current: Option<usize>, len: usize) -> Option<usize> {
    match current {
        None => Some(0),
        Some(i) if i + 1 >= len => None,
        Some(i) => Some(i + 1),
    }
}

fn previous_completion_selection(current: Option<usize>, len: usize) -> Option<usize> {
    match current {
        None => Some(len - 1),
        Some(0) => None,
        Some(i) => Some(i - 1),
    }
}

fn completion_text_for(usage: &str) -> String {
    match usage.find('<') {
        Some(pos) => {
            let base = usage[..pos].trim_end();
            format!("{} ", base)
        }
        None => usage.to_owned(),
    }
}
