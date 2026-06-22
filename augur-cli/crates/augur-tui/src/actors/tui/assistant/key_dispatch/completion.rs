//! Completion refresh and selection helpers for TUI key dispatch.

use crate::actors::tui::tui_actor::TuiHandles;
use crate::domain::tui_state::AppState;
use augur_core::actors::command::handle::CommandHandle;
use augur_core::actors::file_scanner::FileScannerHandle;
use augur_domain::domain::string_newtypes::{PromptText, StringNewtype};

/// Refresh the appropriate completion list after a keypress.
pub(crate) fn refresh_completion_hints(state: &mut AppState, handles: &TuiHandles<'_>) {
    if thinking_mode_picker_open(state) {
        return;
    }
    apply_completion_mode_refresh(state, handles, classify_completion_mode(state));
}

/// Close all completion lists when any are open.
pub fn close_completions_if_open(state: &mut AppState) -> Option<()> {
    if completions_are_empty(&state.prompt.completions) {
        return None;
    }
    clear_all_completions(state);
    Some(())
}

/// Apply the selected completion text to the buffer before submitting.
pub fn apply_selected_completion(state: &mut AppState) {
    if apply_command_completion(state) {
        return;
    }
    if apply_file_completion_if_selected(state) {
        return;
    }
    apply_model_completion(state);
}

enum CompletionMode {
    Model,
    Command,
    File,
    None,
}

fn thinking_mode_picker_open(state: &AppState) -> bool {
    state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id
        .is_some()
}

fn classify_completion_mode(state: &AppState) -> CompletionMode {
    let buffer = state.prompt.buffer.as_str();
    if is_model_completion_prefix(buffer) {
        return CompletionMode::Model;
    }

    let has_at = buffer.contains('@');
    if is_command_completion_prefix(buffer, has_at) {
        return CompletionMode::Command;
    }
    if has_at {
        return CompletionMode::File;
    }
    CompletionMode::None
}

fn apply_completion_mode_refresh(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    mode: CompletionMode,
) {
    match mode {
        CompletionMode::Model => refresh_model_completion_mode(state),
        CompletionMode::Command => refresh_command_completion_mode(state, handles.tools.command),
        CompletionMode::File => refresh_file_completion_mode(state, handles.tools.file_scanner),
        CompletionMode::None => clear_all_completions(state),
    }
}

fn refresh_model_completion_mode(state: &mut AppState) {
    refresh_model_hints(state);
    state.prompt.completions.commands.clear();
    state.prompt.completions.command_selected = None;
    state.prompt.completions.files.clear();
    state.prompt.completions.file_selected = None;
}

fn refresh_command_completion_mode(state: &mut AppState, command: &CommandHandle) {
    refresh_command_hints(state, command);
    state.prompt.completions.files.clear();
    state.prompt.completions.file_selected = None;
    state.prompt.completions.model_picker.items.clear();
    state.prompt.completions.model_picker.selected = None;
}

fn refresh_file_completion_mode(state: &mut AppState, scanner: &FileScannerHandle) {
    refresh_file_hints(state, scanner);
    state.prompt.completions.commands.clear();
    state.prompt.completions.command_selected = None;
    state.prompt.completions.model_picker.items.clear();
    state.prompt.completions.model_picker.selected = None;
}

fn is_model_completion_prefix(buffer: &str) -> bool {
    buffer.starts_with("/model ") || buffer == "/model"
}

fn is_command_completion_prefix(buffer: &str, has_at: bool) -> bool {
    let pipeline_with_file = buffer.starts_with("/run-pipeline") && has_at;
    buffer.starts_with('/') && !pipeline_with_file
}

fn apply_command_completion(state: &mut AppState) -> bool {
    let count = state.prompt.completions.commands.len();
    if count == 0 {
        return false;
    }
    let Some(idx) = state.prompt.completions.command_selected else {
        return false;
    };
    let cmd = state.prompt.completions.commands[idx.min(count - 1)];
    let text = format!("/{}", cmd.name);
    state.prompt.cursor = text.len();
    state.prompt.buffer = text.into();
    true
}

fn apply_file_completion_if_selected(state: &mut AppState) -> bool {
    if state.prompt.completions.files.is_empty() {
        return false;
    }
    if state.prompt.completions.file_selected.is_some() {
        crate::domain::tui_input::apply_file_completion(state);
    }
    true
}

fn apply_model_completion(state: &mut AppState) {
    let count = state.prompt.completions.model_picker.items.len();
    if count == 0 {
        return;
    }
    let Some(idx) = state.prompt.completions.model_picker.selected else {
        return;
    };
    let id = state.prompt.completions.model_picker.items[idx.min(count - 1)]
        .id
        .clone();
    let text = if id.is_empty() {
        "/model".to_owned()
    } else {
        format!("/model {}", id.as_str())
    };
    state.prompt.cursor = text.len();
    state.prompt.buffer = text.into();
}

/// Refresh the command completion list from the current prompt buffer.
pub(crate) fn refresh_command_hints(state: &mut AppState, command: &CommandHandle) {
    let new_completions = command.completions_for(&PromptText::from(state.prompt.buffer.as_str()));
    let old_names: Vec<&str> = state
        .prompt
        .completions
        .commands
        .iter()
        .map(|c| c.name)
        .collect();
    let new_names: Vec<&str> = new_completions.iter().map(|c| c.name).collect();
    if old_names != new_names {
        state.prompt.completions.command_selected = None;
    }
    state.prompt.completions.commands = new_completions;
}

/// Refresh the file completion list from the current prompt buffer.
pub(crate) fn refresh_file_hints(state: &mut AppState, scanner: &FileScannerHandle) {
    let prefix = match state.prompt.buffer.rfind('@') {
        Some(at_pos) => state.prompt.buffer[at_pos + 1..].to_owned(),
        None => {
            state.prompt.completions.files.clear();
            state.prompt.completions.file_selected = None;
            return;
        }
    };
    let new_files = if prefix.ends_with('/') {
        augur_core::actors::file_scanner::file_scanner_actor::scan_directory(
            &augur_domain::domain::string_newtypes::FilePath::new(prefix.as_str()),
        )
    } else {
        scanner.scan(prefix.as_str());
        scanner.latest()
    };
    let old_paths: Vec<&str> = state
        .prompt
        .completions
        .files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    let new_paths: Vec<&str> = new_files.iter().map(|f| f.path.as_str()).collect();
    if old_paths != new_paths {
        state.prompt.completions.file_selected = None;
    }
    state.prompt.completions.files = new_files;
}

/// Refresh the model completion list from the current prompt buffer and cached model list.
pub fn refresh_model_hints(state: &mut AppState) {
    let prefix = model_hint_prefix(state);
    let new_items = filtered_model_items(state, prefix.as_str());
    let old_ids: Vec<&str> = state
        .prompt
        .completions
        .model_picker
        .items
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    let new_ids: Vec<&str> = new_items.iter().map(|m| m.id.as_str()).collect();
    if old_ids != new_ids {
        state.prompt.completions.model_picker.selected =
            preselected_model_hint(state, new_items.as_slice());
    }
    state.prompt.completions.model_picker.items = new_items;
}

/// Clear every completion list and reset their active selections,
/// including the thinking mode picker.
pub(crate) fn clear_all_completions(state: &mut AppState) {
    state.prompt.completions.commands.clear();
    state.prompt.completions.command_selected = None;
    state.prompt.completions.files.clear();
    state.prompt.completions.file_selected = None;
    state.prompt.completions.model_picker.items.clear();
    state.prompt.completions.model_picker.selected = None;
    state.prompt.completions.model_picker.thinking_mode =
        crate::domain::tui_state::ThinkingModeCompletion::default();
}

/// Extract the `/model` completion prefix from the current prompt buffer.
fn model_hint_prefix(state: &AppState) -> String {
    state
        .prompt
        .buffer
        .strip_prefix("/model ")
        .or_else(|| (state.prompt.buffer.as_str() == "/model").then_some(""))
        .unwrap_or("")
        .trim_start()
        .to_owned()
}

/// Build the visible model completion list for the current `/model` prefix.
fn filtered_model_items(
    state: &AppState,
    prefix: &str,
) -> Vec<augur_domain::domain::types::ModelOption> {
    let filtered = filtered_available_models(state, prefix);
    if prefix.is_empty() {
        std::iter::once(auto_model_option())
            .chain(filtered)
            .collect()
    } else {
        filtered.collect()
    }
}

/// Iterate over cached models that match the current `/model` prefix.
fn filtered_available_models<'a>(
    state: &'a AppState,
    prefix: &'a str,
) -> impl Iterator<Item = augur_domain::domain::types::ModelOption> + 'a {
    let prefix_lower = prefix.to_lowercase();
    state
        .prompt
        .models
        .available
        .iter()
        .filter(move |model| model_matches_prefix(model, prefix, prefix_lower.as_str()))
        .cloned()
}

/// Return whether a model id or display label matches the typed `/model` prefix.
fn model_matches_prefix(
    model: &augur_domain::domain::types::ModelOption,
    prefix: &str,
    prefix_lower: &str,
) -> bool {
    prefix.is_empty()
        || model.id.as_str().to_lowercase().contains(prefix_lower)
        || model.display_name.to_lowercase().contains(prefix_lower)
}

/// Construct the synthetic `Auto` model option for a bare `/model` prompt.
fn auto_model_option() -> augur_domain::domain::types::ModelOption {
    use augur_domain::domain::string_newtypes::ModelId;
    use augur_domain::domain::types::ModelOption;

    ModelOption::builder()
        .id(ModelId::new(""))
        .display_name(augur_domain::domain::string_newtypes::ModelLabel::new(
            "Auto",
        ))
        .build()
}

/// Choose the model completion row that should be selected after a refresh.
fn preselected_model_hint(
    state: &AppState,
    new_items: &[augur_domain::domain::types::ModelOption],
) -> Option<usize> {
    let active_id = state.prompt.models.active_id.as_ref();
    new_items
        .iter()
        .position(|model| {
            active_id
                .map(|id| id.as_str() == model.id.as_str())
                .unwrap_or(false)
        })
        .or_else(|| (!new_items.is_empty()).then_some(0))
}

/// Return `true` when every completion collection is currently empty,
/// including when the thinking mode picker is closed.
fn completions_are_empty(completions: &crate::domain::tui_state::PromptCompletions) -> bool {
    completions.commands.is_empty()
        && completions.files.is_empty()
        && completions.model_picker.items.is_empty()
        && completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .is_none()
}
