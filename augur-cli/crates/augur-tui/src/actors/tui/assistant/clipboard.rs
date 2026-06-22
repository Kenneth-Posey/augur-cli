//! Clipboard and selection helpers: paste, selection start/extend, and copy.

use crate::domain::tui_input::insert_paste;
use crate::domain::tui_render::extract_selected_text;
use crate::domain::tui_state::{AppState, OutputSelection, SelectionPoint};
use augur_domain::domain::string_newtypes::{PromptText, StringNewtype};
use crossterm::event::KeyEvent;

/// Read the OS clipboard text and insert it at the current prompt cursor position.
///
/// Uses `arboard::Clipboard` to access the system clipboard. Calls `insert_paste`
/// which normalizes newlines before insertion. Silent no-op when the clipboard
/// is unavailable or returns an error (e.g., no X11 display, empty clipboard).
///
/// Consumers: `handle_mouse_event` (right-click path), `dispatch_chat_key`
/// (RequestPaste action), `handle_plan_mouse_scroll`.
pub(crate) fn paste_from_clipboard(state: &mut AppState) {
    let text = arboard::Clipboard::new()
        .ok()
        .and_then(|mut cb| cb.get_text().ok());
    if let Some(t) = text {
        insert_paste(&mut state.prompt, PromptText::from(t));
    }
}

/// Begin a new text selection anchored at `(row, col)`.
///
/// Both anchor and cursor are set to the same position so no text is selected
/// yet; subsequent `extend_selection` calls will grow the region. Replaces any
/// existing selection.
///
/// Consumers: `handle_mouse_event` on `MouseAction::SelectionStart`.
pub(crate) fn start_selection(state: &mut AppState, pt: SelectionPoint) {
    state.output.selection = Some(OutputSelection {
        anchor: pt,
        cursor: pt,
    });
}

/// Move the cursor endpoint of the active selection to `(row, col)`.
///
/// No-op when there is no active selection (drag before click is discarded).
///
/// Consumers: `handle_mouse_event` on `MouseAction::SelectionExtend`.
pub(crate) fn extend_selection(state: &mut AppState, pt: SelectionPoint) {
    if let Some(sel) = state.output.selection.as_mut() {
        sel.cursor = pt;
    }
}

/// If the 'c' key was pressed and text is selected, copy the selection to the
/// clipboard and clear it.
///
/// Returns `true` when the key was consumed (selection copy performed), preventing
/// 'c' from being appended to the prompt buffer. Returns `false` when no selection
/// is active so normal key handling proceeds.
///
/// Consumers: `dispatch_chat_key` (intercept before `apply_key`).
pub(crate) fn copy_selection_if_c_pressed(state: &mut AppState, key: KeyEvent) -> Option<()> {
    use crossterm::event::{KeyCode, KeyModifiers};
    let is_c = matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        && key.modifiers == KeyModifiers::NONE;
    if !is_c || state.output.selection.is_none() {
        return None;
    }
    if let Some(text) = extract_selected_text(state) {
        let _ = arboard::Clipboard::new().map(|mut cb| cb.set_text(text.into_inner()));
    }
    state.output.selection = None;
    Some(())
}
