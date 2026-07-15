//! Conversation container: primary feed + optional secondary container side panel.
//!
//! Handles the horizontal split between the primary feed and the secondary container
//! when the secondary view is open. When secondary is closed, the primary feed fills
//! the full container width.

use crate::domain::tui_display_state::TuiDisplayState;
use crate::tui::components::primary_feed::{SCROLLBAR_TRACK_COLOR, render_output, render_thinking};
use crate::tui::components::secondary_container::render_secondary_container;
use crate::tui::layout::{
    ConversationArea, compute_secondary_layout, compute_secondary_layout_with_ref,
};
use augur_domain::domain::newtypes::Count;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

const MIN_SECONDARY_PANE_COLS: u16 = 10;

/// Render the conversation container (primary feed + optional secondary container).
///
/// When `state.interaction.panel.secondary_view` is `None`, the primary feed fills the full
/// `conv_area.area`. When secondary is open, splits horizontally using
/// [`compute_secondary_layout`] (or [`compute_secondary_layout_with_ref`] when
/// `conv_area.reference_width` is `Some`), then renders the primary column on the left,
/// a 1-column gutter, and the secondary container on the right.
///
/// Use [`ConversationArea::full`] when `area` already spans the full terminal width.
///
/// The thinking indicator row is rendered as part of the primary column (last row
/// of the primary area).
pub fn render_conversation_container(
    frame: &mut Frame,
    state: &TuiDisplayState,
    conv_area: ConversationArea,
) {
    if state.interaction.panel.secondary_view.is_none() {
        // CRITICAL: Clear secondary_panel_area when secondary view is closed.
        // Prevents stale bounds from intercepting main panel scroll events.
        state
            .output
            .panel_areas
            .secondary_panel_area
            .set(Rect::default());
        render_primary_column(frame, state, conv_area.area);
    } else {
        let layout = match conv_area.reference_width {
            Some(ref_w) => {
                compute_secondary_layout_with_ref(conv_area.area, Count::of(ref_w as usize))
            }
            None => compute_secondary_layout(conv_area.area),
        };
        if layout.secondary_rect.width < MIN_SECONDARY_PANE_COLS {
            state
                .output
                .panel_areas
                .secondary_panel_area
                .set(Rect::default());
            render_primary_column(frame, state, conv_area.area);
            return;
        }
        render_primary_column(frame, state, layout.primary_rect);
        render_gutter(frame, layout.gutter_rect);
        render_secondary_container(frame, state, layout.secondary_rect);
    }
}

/// Render the primary feed column only, ignoring any open secondary container.
///
/// Used by `screens::conversation` as a fallback when the three-pane layout would
/// make the secondary pane too narrow to be useful. Delegates to
/// [`render_primary_column`].
pub(crate) fn render_primary_feed_only(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    render_primary_column(frame, state, area);
}

/// Render the primary feed column: session title header, scrollable output above the thinking row.
///
/// When `state.interaction.session_title` is `Some`, the first row of `area` is reserved for
/// the session title header (dimmed, centered text). The last row of `area` is reserved for the
/// thinking spinner, and the second-to-last row is a blank spacing row. All remaining rows
/// between the title and the blank spacer are the scrollable output pane.
fn render_primary_column(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    let (title_area, remaining) = split_title_area(state, area);
    if let Some(title) = &state.interaction.session_title {
        let dimmed = Paragraph::new(ratatui::text::Text::from(title.to_string()))
            .style(Style::default().add_modifier(Modifier::DIM));
        frame.render_widget(dimmed, title_area);
    }
    let (output_area, thinking_area) = split_output_thinking(remaining);
    render_output(frame, state, output_area);
    render_thinking(frame, state, thinking_area);
}

/// Split `area` into an optional one-row title area and the remaining space.
///
/// When `session_title` is `None`, the title area has zero height and `remaining` is `area`
/// unchanged. When `session_title` is `Some`, the top row of `area` is the title row and
/// `remaining` starts one row below.
fn split_title_area(state: &TuiDisplayState, area: Rect) -> (Rect, Rect) {
    if state.interaction.session_title.is_some() && area.height > 0 {
        let title_rect = Rect { height: 1, ..area };
        let remaining = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };
        (title_rect, remaining)
    } else {
        (Rect { height: 0, ..area }, area)
    }
}

/// Render the vertical gutter separator between the primary and secondary panes.
///
/// Draws a column of `│` characters in `SCROLLBAR_TRACK_COLOR` for the full height
/// of `area`. No-ops when `area.width == 0`.
fn render_gutter(frame: &mut Frame, area: Rect) {
    if area.width == 0 {
        return;
    }
    let lines: Vec<Line> = (0..area.height)
        .map(|_| {
            Line::from(Span::styled(
                "│",
                Style::default().fg(SCROLLBAR_TRACK_COLOR),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)), area);
}

/// Split `area` into (output_area, thinking_area).
///
/// The last row of `area` is the thinking spinner row.
/// The second-to-last row is a dedicated blank spacing row.
/// All rows above are the output area.
///
/// When `area.height` is 0, both returned rects have zero height.
/// When `area.height` is 1, thinking takes the single row and output has height 0.
fn split_output_thinking(area: Rect) -> (Rect, Rect) {
    if area.height == 0 {
        return (area, Rect { height: 0, ..area });
    }
    if area.height == 1 {
        let thinking = Rect {
            y: area.y,
            height: 1,
            ..area
        };
        let output = Rect { height: 0, ..area };
        return (output, thinking);
    }
    let thinking = Rect {
        y: area.y + area.height.saturating_sub(1),
        height: 1,
        ..area
    };
    // Reserve 2 rows: one blank spacer and one thinking row.
    let output_height = area.height.saturating_sub(2).max(1);
    let output = Rect {
        height: output_height,
        ..area
    };
    (output, thinking)
}
