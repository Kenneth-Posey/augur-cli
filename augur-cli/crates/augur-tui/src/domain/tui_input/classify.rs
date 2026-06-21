//! Event classification helpers for TUI input handling.

use super::*;

/// Classify a mouse event against the output pane.
pub fn classify_mouse(event: MouseEvent, output_area: Rect) -> MouseAction {
    if is_right_click(event.kind) {
        return MouseAction::RightClick;
    }
    if mouse_in_output_area(&event, output_area) {
        return classify_output_area_mouse(event);
    }
    classify_outside_output_area_mouse(event.kind)
}

/// Classify a chat-mode key event.
pub fn classify_key(event: KeyEvent) -> KeyAction {
    classify_submission_key(&event)
        .or_else(|| classify_focus_key(&event))
        .or_else(|| classify_navigation_key(&event))
        .or_else(|| classify_scroll_key(&event))
        .or_else(|| classify_character_key(&event))
        .unwrap_or_else(|| classify_fallback_key(&event))
}

/// Classify a key event for the session picker.
pub fn classify_picker_key(event: KeyEvent) -> PickerKeyAction {
    classify_picker_navigation_key(&event)
        .or_else(|| classify_picker_management_key(&event))
        .or_else(|| classify_picker_quit_key(&event))
        .unwrap_or(PickerKeyAction::Ignored)
}

/// Classify a key event for the query overlay.
pub fn classify_query_key(event: KeyEvent) -> QueryKeyAction {
    classify_query_navigation_key(&event)
        .or_else(|| classify_query_control_key(&event))
        .or_else(|| classify_query_character_key(&event))
        .unwrap_or(QueryKeyAction::Ignored)
}

fn is_right_click(kind: MouseEventKind) -> bool {
    matches!(kind, MouseEventKind::Down(MouseButton::Right))
}

fn mouse_in_output_area(event: &MouseEvent, output_area: Rect) -> bool {
    event.row >= output_area.y
        && event.row < output_area.y + output_area.height
        && event.column >= output_area.x
        && event.column < output_area.x + output_area.width
}

fn classify_output_area_mouse(event: MouseEvent) -> MouseAction {
    classify_output_scroll_mouse(event.kind)
        .or_else(|| classify_output_selection_mouse(event))
        .unwrap_or(MouseAction::Ignored)
}

fn classify_outside_output_area_mouse(kind: MouseEventKind) -> MouseAction {
    match kind {
        MouseEventKind::Down(MouseButton::Left) => MouseAction::ClearSelection,
        _ => MouseAction::Ignored,
    }
}

fn mouse_selection_start(event: MouseEvent) -> MouseAction {
    MouseAction::SelectionStart {
        row: event.row,
        col: event.column,
    }
}

fn mouse_selection_extend(event: MouseEvent) -> MouseAction {
    MouseAction::SelectionExtend {
        row: event.row,
        col: event.column,
    }
}

fn classify_submission_key(event: &KeyEvent) -> Option<KeyAction> {
    classify_submission_primary_key(event).or_else(|| classify_submission_edit_key(event))
}

fn classify_focus_key(event: &KeyEvent) -> Option<KeyAction> {
    classify_focus_tab_key(event)
        .or_else(|| classify_focus_feed_key(event))
        .or_else(|| classify_focus_control_key(event))
}

fn classify_navigation_key(event: &KeyEvent) -> Option<KeyAction> {
    classify_navigation_horizontal_key(event)
        .or_else(|| classify_navigation_vertical_key(event))
        .or_else(|| classify_navigation_boundary_key(event))
}

fn classify_scroll_key(event: &KeyEvent) -> Option<KeyAction> {
    classify_scroll_page_key(event).or_else(|| classify_scroll_half_key(event))
}

fn classify_character_key(event: &KeyEvent) -> Option<KeyAction> {
    match (event.code, event.modifiers) {
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            Some(KeyAction::AppendChar(c))
        }
        _ => None,
    }
}

fn classify_fallback_key(event: &KeyEvent) -> KeyAction {
    match event.code {
        KeyCode::Esc => KeyAction::CancelThinking,
        _ => KeyAction::Ignored,
    }
}

fn classify_picker_navigation_key(event: &KeyEvent) -> Option<PickerKeyAction> {
    match event.code {
        KeyCode::Up => Some(PickerKeyAction::SelectUp),
        KeyCode::Down => Some(PickerKeyAction::SelectDown),
        KeyCode::Enter => Some(PickerKeyAction::Confirm),
        _ => None,
    }
}

fn classify_picker_management_key(event: &KeyEvent) -> Option<PickerKeyAction> {
    if picker_delete_key(event) {
        return Some(PickerKeyAction::Delete);
    }
    if picker_new_session_key(event) {
        return Some(PickerKeyAction::NewSession);
    }
    None
}

fn classify_picker_quit_key(event: &KeyEvent) -> Option<PickerKeyAction> {
    matches!(
        (event.code, event.modifiers),
        (KeyCode::Char('c'), KeyModifiers::CONTROL)
    )
    .then_some(PickerKeyAction::Quit)
}

fn picker_delete_key(event: &KeyEvent) -> bool {
    matches!(
        (event.code, event.modifiers),
        (KeyCode::Char('d'), KeyModifiers::NONE) | (KeyCode::Char('D'), _)
    )
}

fn picker_new_session_key(event: &KeyEvent) -> bool {
    matches!(
        (event.code, event.modifiers),
        (KeyCode::Char('n'), KeyModifiers::NONE) | (KeyCode::Char('N'), _)
    )
}

fn classify_query_navigation_key(event: &KeyEvent) -> Option<QueryKeyAction> {
    match event.code {
        KeyCode::Up => Some(QueryKeyAction::SelectUp),
        KeyCode::Down => Some(QueryKeyAction::SelectDown),
        KeyCode::Enter => Some(QueryKeyAction::Submit),
        _ => None,
    }
}

fn classify_query_control_key(event: &KeyEvent) -> Option<QueryKeyAction> {
    if matches!(
        (event.code, event.modifiers),
        (KeyCode::Char('c'), KeyModifiers::CONTROL)
    ) {
        return Some(QueryKeyAction::Quit);
    }
    matches!(event.code, KeyCode::Backspace).then_some(QueryKeyAction::Backspace)
}

fn classify_query_character_key(event: &KeyEvent) -> Option<QueryKeyAction> {
    match (event.code, event.modifiers) {
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            Some(QueryKeyAction::AppendFreeform(c))
        }
        _ => None,
    }
}

fn classify_output_scroll_mouse(kind: MouseEventKind) -> Option<MouseAction> {
    match kind {
        MouseEventKind::ScrollUp => Some(MouseAction::ScrollUp(MOUSE_SCROLL_LINES)),
        MouseEventKind::ScrollDown => Some(MouseAction::ScrollDown(MOUSE_SCROLL_LINES)),
        _ => None,
    }
}

fn classify_output_selection_mouse(event: MouseEvent) -> Option<MouseAction> {
    match event.kind {
        MouseEventKind::Down(MouseButton::Left) => Some(mouse_selection_start(event)),
        MouseEventKind::Drag(MouseButton::Left) => Some(mouse_selection_extend(event)),
        _ => None,
    }
}

fn classify_submission_primary_key(event: &KeyEvent) -> Option<KeyAction> {
    if matches!(event.code, KeyCode::Enter) {
        return Some(KeyAction::Submit);
    }
    matches!(
        (event.code, event.modifiers),
        (KeyCode::Char('c'), KeyModifiers::CONTROL)
    )
    .then_some(KeyAction::Quit)
}

fn classify_submission_edit_key(event: &KeyEvent) -> Option<KeyAction> {
    match event.code {
        KeyCode::Backspace => Some(KeyAction::Backspace),
        KeyCode::Delete => Some(KeyAction::Delete),
        _ => None,
    }
}

fn classify_focus_tab_key(event: &KeyEvent) -> Option<KeyAction> {
    match event.code {
        KeyCode::Tab => Some(KeyAction::ToggleAskFocus),
        KeyCode::BackTab => Some(KeyAction::ShiftTab),
        _ => None,
    }
}

fn classify_focus_feed_key(event: &KeyEvent) -> Option<KeyAction> {
    match (event.code, event.modifiers) {
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => Some(KeyAction::ToggleAgentFeed),
        (KeyCode::Char('o'), KeyModifiers::CONTROL) => Some(KeyAction::AgentFeedPrev),
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => Some(KeyAction::AgentFeedNext),
        _ => None,
    }
}

fn classify_focus_control_key(event: &KeyEvent) -> Option<KeyAction> {
    match (event.code, event.modifiers) {
        (KeyCode::Char('v'), KeyModifiers::CONTROL) => Some(KeyAction::RequestPaste),
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => Some(KeyAction::CloseSecondaryPanel),
        _ => None,
    }
}

fn classify_navigation_horizontal_key(event: &KeyEvent) -> Option<KeyAction> {
    match event.code {
        KeyCode::Left => Some(KeyAction::CursorLeft),
        KeyCode::Right => Some(KeyAction::CursorRight),
        _ => None,
    }
}

fn classify_navigation_vertical_key(event: &KeyEvent) -> Option<KeyAction> {
    match event.code {
        KeyCode::Up => Some(KeyAction::CompletionUp),
        KeyCode::Down => Some(KeyAction::CompletionDown),
        _ => None,
    }
}

fn classify_navigation_boundary_key(event: &KeyEvent) -> Option<KeyAction> {
    match event.code {
        KeyCode::Home => Some(KeyAction::CursorHome),
        KeyCode::End => Some(KeyAction::CursorEnd),
        _ => None,
    }
}

fn classify_scroll_page_key(event: &KeyEvent) -> Option<KeyAction> {
    match event.code {
        KeyCode::PageUp => Some(KeyAction::ScrollUp(KEY_SCROLL_LINES_PAGE)),
        KeyCode::PageDown => Some(KeyAction::ScrollDown(KEY_SCROLL_LINES_PAGE)),
        _ => None,
    }
}

fn classify_scroll_half_key(event: &KeyEvent) -> Option<KeyAction> {
    match (event.code, event.modifiers) {
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            Some(KeyAction::ScrollUp(KEY_SCROLL_LINES_HALF))
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            Some(KeyAction::ScrollDown(KEY_SCROLL_LINES_HALF))
        }
        _ => None,
    }
}
