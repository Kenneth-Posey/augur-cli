//! Ratatui rendering shell. Accepts &TuiDisplayState and &mut Frame; no terminal I/O.
//!
//! The shell dispatches to screen-specific renderers in `screens/`:
//! - `AppScreen::SessionSelector` → `screens::session_selector::render_session_selector`
//! - `AppScreen::Conversation` → `screens::conversation::render_conversation`
//!
//! All component-level rendering lives in `components/` and `screens/`.

use crate::domain::tui_display_state::TuiDisplayState;
use crate::domain::tui_state::AppScreen;
use crate::tui::layout::split_controls_area;
use crate::tui::screens::conversation::render_conversation;
use crate::tui::screens::session_selector::render_session_selector;
use ratatui::Frame;

// Re-exports so existing tests can import from `crate::tui::render` without change.
#[allow(unused_imports)]
pub use crate::domain::tui_render::{
    LineCharPosition, RenderSlice, RenderSliceInput, ScreenPosToLineCharInput,
    compute_render_slice, extract_selected_text, format_response_prefix, line_display_rows,
    rendered_line_text, screen_pos_to_line_char,
};
#[allow(unused_imports)]
pub use crate::tui::components::footer::{controls_row_hint, status_left, status_right};
#[allow(unused_imports)]
pub use crate::tui::components::primary_feed::{scroll_marker_row, separator_line};
// Query-helper re-exports: still accessible at crate::tui::render for test compat.
#[allow(unused_imports)]
pub use crate::tui::screens::conversation::{build_inline_choice_lines, split_question_lines};

/// Render the full TUI layout based on the current `AppScreen`.
///
/// Dispatches to the session selector screen or the conversation screen.
/// For the conversation screen the full terminal area is passed so that
/// `render_conversation` can carve off the bottom controls row internally.
/// The session selector receives only the main area (controls row hidden).
///
/// Parameters:
/// - `frame`: the ratatui frame for this draw pass.
/// - `display`: the cloned display state for this frame.
///
/// Side effects: writes widgets into `frame`; no I/O.
pub fn render_with_overlays(frame: &mut Frame, display: &TuiDisplayState) {
    let full_area = frame.area();
    match &display.interaction.screen {
        AppScreen::SessionSelector(ps) => {
            let (main_area, _) = split_controls_area(full_area);
            render_session_selector(frame, ps, main_area);
        }
        AppScreen::Conversation => render_conversation(frame, display, full_area),
    }
}
