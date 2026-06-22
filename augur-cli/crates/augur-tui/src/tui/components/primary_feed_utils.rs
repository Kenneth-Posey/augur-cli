//! Pure text and scroll utility functions for the primary feed output pane.
//!
//! Extracted from `primary_feed.rs` to keep per-file line counts within limits.
//! All functions here are free of `Frame` rendering concerns; they compute
//! text layout, scroll geometry, or string formatting from plain data.

use crate::domain::tui_render::SCROLLBAR_WIDTH;
use crate::domain::tui_state::OutputSelection;
use augur_domain::domain::newtypes::{Count, IsVisible, NumericNewtype};
use ratatui::layout::Rect;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeparatorText(String);

impl fmt::Display for SeparatorText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, bon::Builder)]
pub struct ScrollRenderContext {
    pub total_lines: usize,
    pub(crate) visible_lines: usize,
    pub(crate) scroll_offset: usize,
    pub(crate) indicator_height: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollMarker {
    pub row: Count,
    pub visible: IsVisible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, bon::Builder)]
pub(crate) struct SelectionBounds {
    pub(crate) y_start: u16,
    pub(crate) x_start: u16,
    pub(crate) y_end: u16,
    pub(crate) x_end: u16,
}

/// Produce the horizontal-rule string for a separator row.
///
/// Returns a string of exactly `width` box-drawing `─` (U+2500) characters.
/// Used by `render_separator` and testable independently of frame rendering.
pub fn separator_line(width: Count) -> SeparatorText {
    SeparatorText("─".repeat(width.inner()))
}

/// Split the output area into content and scrollbar columns.
///
/// Returns `(content_area, scrollbar_area)`. When `area.width <= SCROLLBAR_WIDTH`,
/// the full area is returned as `content_area` and `scrollbar_area` is a zero-size
/// default rect so the scrollbar renders as a no-op.
pub fn split_output_area(area: Rect) -> (Rect, Rect) {
    if area.width <= SCROLLBAR_WIDTH {
        return (area, Rect::default());
    }
    let content = Rect {
        width: area.width - SCROLLBAR_WIDTH,
        ..area
    };
    let scrollbar = Rect {
        x: area.x + area.width - SCROLLBAR_WIDTH,
        width: SCROLLBAR_WIDTH,
        ..area
    };
    (content, scrollbar)
}

/// Compute the row index and visibility of the scroll-position marker.
///
/// Returns `(marker_row, show_marker)`. `show_marker` is `false` when all content
/// fits within the visible area or `indicator_height` is zero - no scrolling is
/// possible so no marker is needed. Otherwise the marker row is derived from the
/// current `scroll_offset` as a fraction of the maximum scrollable range, mapped
/// onto the indicator height so that:
/// - `scroll_offset == 0` (bottom of conversation) → marker at `height - 1`
/// - `scroll_offset == max_offset` (top of conversation) → marker at `0`
///
/// Made `pub(crate)` so tests can verify the position formula independently.
pub fn scroll_marker_row(context: ScrollRenderContext) -> ScrollMarker {
    if context.total_lines <= context.visible_lines || context.indicator_height == 0 {
        return ScrollMarker {
            row: Count::ZERO,
            visible: IsVisible::no(),
        };
    }
    let max_offset = context.total_lines.saturating_sub(context.visible_lines);
    let ratio = context.scroll_offset as f64 / max_offset as f64;
    let row =
        ((1.0 - ratio) * (context.indicator_height.saturating_sub(1)) as f64).round() as usize;
    ScrollMarker {
        row: Count::of(row.min(context.indicator_height.saturating_sub(1))),
        visible: IsVisible::yes(),
    }
}

/// Normalize selection endpoints to `(start_row, start_col, end_row, end_col)`.
///
/// Compares `(anchor.row, anchor.col)` against `(cursor.row, cursor.col)` and
/// returns them in forward order. Clamps to the `content_area` boundaries so
/// overlay rendering never exceeds the output content zone.
///
/// Callers: `apply_selection_overlay`.
pub(crate) fn normalize_selection(sel: &OutputSelection, content_area: Rect) -> SelectionBounds {
    let (ar, ac) = (sel.anchor.row, sel.anchor.col);
    let (cr, cc) = (sel.cursor.row, sel.cursor.col);
    let ((sr, sc), (er, ec)) = if (ar, ac) <= (cr, cc) {
        ((ar, ac), (cr, cc))
    } else {
        ((cr, cc), (ar, ac))
    };
    let x_start = sc.max(content_area.x);
    let x_end = ec.min(content_area.x + content_area.width - 1);
    let y_start = sr.max(content_area.y);
    let y_end = er.min(content_area.y + content_area.height - 1);
    SelectionBounds::builder()
        .y_start(y_start)
        .x_start(x_start)
        .y_end(y_end)
        .x_end(x_end)
        .build()
}
