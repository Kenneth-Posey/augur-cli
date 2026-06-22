//! Output-selection helpers shared between the actor and render shell.

use super::{
    RenderSlice, RenderSliceInput, SCROLLBAR_WIDTH, compute_render_slice, line_display_rows,
    rendered_line_text,
};
use crate::domain::tui_state::{AppState, OutputLine, SelectionPoint};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::{SelectedText, StringNewtype};
use ratatui::layout::{Position, Rect};

/// Input bundle for mapping a screen position into rendered output text.
#[derive(Clone, Copy, bon::Builder)]
pub struct ScreenPosToLineCharInput<'a> {
    /// Screen-space position within the terminal.
    pub(crate) screen_pos: Position,
    /// Full logical output line set backing the rendered paragraph.
    pub(crate) lines: &'a [OutputLine],
    /// Rect occupied by the wrapped content area (excluding the scrollbar).
    pub(crate) content_area: Rect,
    /// Wrapped render slice active for the paragraph.
    pub(crate) render_slice: RenderSlice,
}

/// Line/character position within rendered output text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineCharPosition {
    pub line_index: usize,
    pub char_offset: usize,
}

/// Map a screen position to a rendered line/character position.
pub fn screen_pos_to_line_char(input: ScreenPosToLineCharInput<'_>) -> LineCharPosition {
    if input.lines.is_empty() {
        return LineCharPosition {
            line_index: 0,
            char_offset: 0,
        };
    }
    let width = input.content_area.width as usize;
    if width == 0 {
        return LineCharPosition {
            line_index: input.render_slice.start,
            char_offset: 0,
        };
    }
    let rendered_lines = &input.lines[input.render_slice.start..input.render_slice.end];
    if rendered_lines.is_empty() {
        let line_index = input
            .render_slice
            .start
            .min(input.lines.len().saturating_sub(1));
        return LineCharPosition {
            line_index,
            char_offset: 0,
        };
    }

    let target = rendered_target_position(input);
    if let Some(position) = map_target_to_rendered_lines(
        rendered_lines,
        RenderedLineSearchInput {
            input,
            target,
            width,
        },
    ) {
        return position;
    }
    last_rendered_position(input.lines, input.render_slice)
}

#[derive(Clone, Copy)]
struct RenderedLineSearchInput<'a> {
    input: ScreenPosToLineCharInput<'a>,
    target: TargetPosition,
    width: usize,
}

fn map_target_to_rendered_lines(
    rendered_lines: &[OutputLine],
    search: RenderedLineSearchInput<'_>,
) -> Option<LineCharPosition> {
    let mut display_rows_so_far = 0usize;
    for (i, line) in rendered_lines.iter().enumerate() {
        if let Some(position) = line_position_for_target_row(
            line,
            LinePositionContext {
                line_offset: i,
                search,
                display_rows_so_far,
            },
        ) {
            return Some(position);
        }
        display_rows_so_far +=
            line_display_rows(&rendered_line_text(line), Count::new(search.width)).inner();
    }
    None
}

#[derive(Clone, Copy)]
struct LinePositionContext<'a> {
    line_offset: usize,
    search: RenderedLineSearchInput<'a>,
    display_rows_so_far: usize,
}

fn line_position_for_target_row(
    line: &OutputLine,
    ctx: LinePositionContext<'_>,
) -> Option<LineCharPosition> {
    let rendered = rendered_line_text(line);
    let rows = line_display_rows(&rendered, Count::new(ctx.search.width));
    if ctx.display_rows_so_far + rows.inner() <= ctx.search.target.row {
        return None;
    }
    let row_within_line = ctx.search.target.row - ctx.display_rows_so_far;
    let char_offset =
        (row_within_line * ctx.search.width + ctx.search.target.col).min(rendered.chars().count());
    Some(LineCharPosition {
        line_index: ctx.search.input.render_slice.start + ctx.line_offset,
        char_offset,
    })
}

/// Extract the text covered by the active output selection.
pub fn extract_selected_text(state: &AppState) -> Option<SelectedText> {
    let sel = state.output.selection.as_ref()?;
    let content_area = selection_content_area(state.output.panel_areas.output_area.get())?;
    let render_slice = selection_render_slice(state, content_area);
    let anchor = selection_endpoint(SelectionEndpointInput {
        point: sel.anchor,
        lines: &state.output.lines,
        content_area,
        render_slice,
    });
    let cursor = selection_endpoint(SelectionEndpointInput {
        point: sel.cursor,
        lines: &state.output.lines,
        content_area,
        render_slice,
    });
    let (start_pos, end_pos) = ordered_selection(anchor, cursor);
    Some(SelectedText::from(extract_selection_range(
        &state.output.lines,
        start_pos,
        end_pos,
    )))
}

#[derive(Clone, Copy)]
struct SelectionEndpointInput<'a> {
    point: SelectionPoint,
    lines: &'a [OutputLine],
    content_area: Rect,
    render_slice: RenderSlice,
}

#[derive(Clone, Copy)]
struct TargetPosition {
    row: usize,
    col: usize,
}

fn selection_content_area(output_area: Rect) -> Option<Rect> {
    if output_area.width <= SCROLLBAR_WIDTH {
        return None;
    }
    let mut content_area = output_area;
    content_area.width -= SCROLLBAR_WIDTH;
    Some(content_area)
}

fn selection_render_slice(state: &AppState, content_area: Rect) -> RenderSlice {
    compute_render_slice(
        RenderSliceInput::builder()
            .lines(&state.output.lines)
            .visible_rows(Count::new(content_area.height as usize))
            .scroll_offset(state.output.scroll_offset.get())
            .content_width(Count::new(content_area.width as usize))
            .build(),
    )
}

fn selection_endpoint(input: SelectionEndpointInput<'_>) -> LineCharPosition {
    screen_pos_to_line_char(
        ScreenPosToLineCharInput::builder()
            .screen_pos(Position::new(input.point.col, input.point.row))
            .lines(input.lines)
            .content_area(input.content_area)
            .render_slice(input.render_slice)
            .build(),
    )
}

fn rendered_target_position(input: ScreenPosToLineCharInput<'_>) -> TargetPosition {
    TargetPosition {
        row: input.screen_pos.y.saturating_sub(input.content_area.y) as usize
            + input.render_slice.para_scroll as usize,
        col: input.screen_pos.x.saturating_sub(input.content_area.x) as usize,
    }
}

fn last_rendered_position(lines: &[OutputLine], render_slice: RenderSlice) -> LineCharPosition {
    let last_idx = render_slice.end - 1;
    let last_text = rendered_line_text(&lines[last_idx]);
    LineCharPosition {
        line_index: last_idx,
        char_offset: last_text.chars().count(),
    }
}

fn ordered_selection(
    anchor: LineCharPosition,
    cursor: LineCharPosition,
) -> (LineCharPosition, LineCharPosition) {
    if anchor <= cursor {
        (anchor, cursor)
    } else {
        (cursor, anchor)
    }
}

fn extract_selection_range(
    lines: &[OutputLine],
    start_pos: LineCharPosition,
    end_pos: LineCharPosition,
) -> String {
    if start_pos.line_index == end_pos.line_index {
        return extract_line_segment(
            rendered_line_text(&lines[start_pos.line_index]).as_str(),
            start_pos.char_offset,
            end_pos.char_offset,
        );
    }

    let mut result = String::new();
    for line_idx in start_pos.line_index..=end_pos.line_index {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(&extract_line_segment(
            rendered_line_text(&lines[line_idx]).as_str(),
            line_segment_start(line_idx, start_pos),
            line_segment_end(lines, line_idx, end_pos),
        ));
    }
    result
}

fn extract_line_segment(text: &str, from: usize, to: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[from..to.min(chars.len())].iter().collect()
}

fn line_segment_start(line_idx: usize, start_pos: LineCharPosition) -> usize {
    if line_idx == start_pos.line_index {
        start_pos.char_offset
    } else {
        0
    }
}

fn line_segment_end(lines: &[OutputLine], line_idx: usize, end_pos: LineCharPosition) -> usize {
    if line_idx == end_pos.line_index {
        end_pos.char_offset
    } else {
        rendered_line_text(&lines[line_idx]).chars().count()
    }
}
