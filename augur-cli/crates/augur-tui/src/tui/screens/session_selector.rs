//! Session selector screen rendering.
//!
//! Wraps the picker rendering logic from `picker.rs` under the `render_session_selector`
//! name that the shell dispatcher uses.

use crate::domain::tui_state::PickerState;
use crate::tui::picker::render_picker;
use ratatui::Frame;
use ratatui::layout::Rect;

/// Render the session selector screen into `area`.
///
/// Delegates to [`crate::tui::picker::render_picker`].
/// When sessions are available, renders a navigable list; when the list is empty,
/// shows a centered prompt to start a new session.
///
/// Called by the shell dispatcher when `AppScreen::SessionSelector` is active.
pub(crate) fn render_session_selector(frame: &mut Frame, state: &PickerState, area: Rect) {
    render_picker(frame, state, area);
}
