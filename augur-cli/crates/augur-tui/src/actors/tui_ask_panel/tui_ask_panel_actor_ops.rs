//! Private helper operations for the TUI ask-panel actor.

use super::handle::ScrollDelta;
use crate::domain::tui_state::AskPanelState;

/// Apply a signed scroll delta to the ask panel state when the panel is open.
pub(super) fn apply_scroll(state: &mut Option<AskPanelState>, delta: ScrollDelta) {
    if let Some(s) = state.as_mut() {
        s.scroll = delta.apply_to(s.scroll);
    }
}
