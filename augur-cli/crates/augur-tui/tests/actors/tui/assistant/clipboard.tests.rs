use crate::domain::string_newtypes::{EndpointName, StringNewtype};
use crate::domain::tui_state::{AppScreen, AppState, OutputLine, OutputSelection, SelectionPoint};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::layout::Rect;

fn selection_state(lines: Vec<OutputLine>) -> AppState {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.output.lines = lines;
    state.output.panel_areas.output_area.set(Rect {
        x: 0,
        y: 0,
        width: 21,
        height: 4,
    });
    state
}

fn select_range(state: &mut AppState, anchor: (u16, u16), cursor: (u16, u16)) {
    state.output.selection = Some(OutputSelection {
        anchor: SelectionPoint {
            row: anchor.0,
            col: anchor.1,
        },
        cursor: SelectionPoint {
            row: cursor.0,
            col: cursor.1,
        },
    });
}

/// Verifies that paste_from_clipboard does not panic regardless of whether
/// the clipboard is accessible. In headless CI environments arboard may fail
/// silently; in environments with a display the clipboard may hold arbitrary
/// content. Either way, the function must not panic.
#[test]
fn paste_from_clipboard_does_not_panic() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    // Should not panic whether or not arboard can initialize.
    super::paste_from_clipboard(&mut state);
    // No assertion on buffer contents - clipboard state is environment-dependent.
}

/// Verifies that start_selection anchors a new selection at the clicked point.
#[test]
fn start_selection_sets_anchor_and_cursor_to_same_point() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    super::start_selection(&mut state, SelectionPoint { row: 5, col: 9 });

    assert_eq!(
        state.output.selection,
        Some(OutputSelection {
            anchor: SelectionPoint { row: 5, col: 9 },
            cursor: SelectionPoint { row: 5, col: 9 },
        })
    );
}

/// Verifies that extend_selection moves only the cursor endpoint when a
/// selection is already active.
#[test]
fn extend_selection_updates_cursor_for_active_selection() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.output.selection = Some(OutputSelection {
        anchor: SelectionPoint { row: 3, col: 4 },
        cursor: SelectionPoint { row: 3, col: 4 },
    });

    super::extend_selection(&mut state, SelectionPoint { row: 8, col: 15 });

    assert_eq!(
        state.output.selection,
        Some(OutputSelection {
            anchor: SelectionPoint { row: 3, col: 4 },
            cursor: SelectionPoint { row: 8, col: 15 },
        })
    );
}

/// Verifies that extend_selection is a no-op when no selection exists.
#[test]
fn extend_selection_without_active_selection_is_noop() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    super::extend_selection(&mut state, SelectionPoint { row: 2, col: 7 });

    assert!(state.output.selection.is_none());
}

/// Verifies that pressing plain `c` with a selection consumes the key and clears
/// the selection after attempting the clipboard copy.
#[test]
fn copy_selection_if_plain_c_consumes_key_and_clears_selection() {
    let mut state = selection_state(vec![OutputLine::plain("abcdef")]);
    select_range(&mut state, (0, 1), (0, 4));

    let consumed = super::copy_selection_if_c_pressed(
        &mut state,
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        },
    );

    assert_eq!(consumed, Some(()));
    assert!(
        state.output.selection.is_none(),
        "copy path must clear the active selection"
    );
}
