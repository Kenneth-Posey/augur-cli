//! Shared TUI render contracts and pure text-layout helpers.
//!
//! This module is owned by the shared domain layer so both the TUI shell and
//! the TUI actor can depend on the same contracts without creating an
//! `actors -> tui` reverse dependency.

mod render_slice;
mod selection;

pub use render_slice::{
    compute_render_slice, format_response_prefix, line_display_rows, rendered_line_text,
    RenderSlice, RenderSliceInput,
};
pub use selection::{
    extract_selected_text, screen_pos_to_line_char, LineCharPosition, ScreenPosToLineCharInput,
};

/// Function contract for rendering the current display state into a Ratatui frame.
///
/// `wiring.rs` injects the concrete renderer from `src/tui/`, while the actor
/// runtime depends only on this lower-tier function signature. The renderer
/// accepts a [`crate::domain::tui_display_state::TuiDisplayState`] projection so that the actor layer (`L8`) never
/// imports directly from the render layer (`L10`).
pub type AppRenderer =
    for<'a> fn(&mut ratatui::Frame<'a>, &crate::domain::tui_display_state::TuiDisplayState);

/// Width in columns reserved for the scroll-position indicator on the right edge
/// of the output pane.
pub(crate) const SCROLLBAR_WIDTH: u16 = 1;
