//! Primary feed (output pane) rendering: output lines, scroll indicator, separator, thinking row.

use super::primary_feed_utils::{ScrollRenderContext, normalize_selection};
use crate::domain::tui_display_state::TuiDisplayState;
use crate::domain::tui_render::RenderSliceInput;
use crate::domain::tui_state::{LineKind, OutputLine, OutputSelection};
use augur_domain::domain::newtypes::{Count, NumericNewtype, ScrollOffset};
use augur_domain::domain::string_newtypes::StringNewtype;
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

pub use super::primary_feed_utils::{scroll_marker_row, separator_line, split_output_area};
#[allow(unused_imports)]
pub(crate) use crate::domain::tui_render::{
    compute_render_slice, format_response_prefix, line_display_rows, rendered_line_text,
};

/// Old and new content-area widths passed to `recalculate_scroll_for_width_change`.
///
/// Groups the two width values so the function stays within the three-parameter limit.
#[derive(Debug, Clone, Copy)]
struct WidthChange {
    old: usize,
    new: usize,
}

/// Foreground color for the scroll track `│` characters.
///
/// Dark gray keeps the track visually present but receded so it does not compete
/// with output content. The contrasting marker uses `SCROLLBAR_MARKER_COLOR`.
pub(crate) const SCROLLBAR_TRACK_COLOR: Color = Color::DarkGray;

/// Foreground color for the scroll-position marker `█` character.
///
/// Cyan contrasts with both dark and light terminal backgrounds and with the
/// dark-gray track, making the marker immediately visible without being harsh.
pub(crate) const SCROLLBAR_MARKER_COLOR: Color = Color::Cyan;

/// Braille spinner frames cycled by `render_thinking` while the agent is working.
///
/// Ten distinct Braille pattern characters that form a smooth rotating animation
/// at the ~100 ms tick rate driven by `AgentStatus.spinner_tick`.
pub(crate) const BRAILLE_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Background color applied to user-submitted messages in the output pane.
///
/// A very dark green (`rgb(0, 25, 0)`) visually separates user messages from
/// agent output on dark terminals. Applied to both the content span and the
/// timestamp prefix span so the entire line row has a consistent background.
/// Consumers: `output_line_to_ratatui`.
const USER_INPUT_BG: Color = Color::Rgb(0, 50, 0);

/// Background color applied to selected text in the output pane.
///
/// Blue-4 (indexed 25) provides visible contrast on both dark and light
/// terminals while remaining distinct from the user-input background.
/// Applied as an overlay via `frame.buffer_mut()` after paragraph rendering.
const SELECTION_BG: Color = Color::Indexed(25);

#[derive(Clone, Copy)]
pub(crate) struct ScrollIndicatorRenderContext {
    pub(crate) area: Rect,
    pub(crate) scroll: ScrollRenderContext,
}

/// Context for applying selection overlay to output text.
#[derive(Clone, Copy)]
pub(crate) struct SelectionRenderContext {
    pub(crate) selection: OutputSelection,
    pub(crate) content_area: Rect,
}

/// Render a full-width horizontal separator line into `area`.
pub(crate) fn render_separator(frame: &mut Frame, area: Rect) {
    let line = separator_line(Count::of(area.width as usize));
    frame.render_widget(Paragraph::new(line.to_string()), area);
}

/// Render the thinking status row.
///
/// Shows a rotating Braille spinner and the current `thinking_label` when
/// ANY of the following is true:
/// - The main conversation is thinking (`state.agent.thinking.is_active`).
/// - The agent feed panel has an active task.
/// - The ask panel is waiting for a response (`ask_panel.thinking`).
///
/// This ensures the main conversation spinner indicates that work is happening
/// anywhere in the session - including background tasks and the ask side-channel.
///
/// Renders nothing (empty row) when all are inactive so the row is visually invisible.
pub(crate) fn render_thinking(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    let agent_feed_active = bool::from(state.any_agent_feed_active());
    let ask_panel_thinking: bool = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .map(|p| p.thinking.into())
        .unwrap_or(false);
    let main_thinking = state.agent.thinking.is_active;
    let any_active = main_thinking.into() || agent_feed_active || ask_panel_thinking;
    if !any_active {
        return;
    }
    let frame_idx = (state.agent.thinking.spinner_tick as usize) % BRAILLE_FRAMES.len();
    let spinner = BRAILLE_FRAMES[frame_idx];
    let text = format!("{} {}", spinner, state.agent.thinking.label);
    let paragraph = Paragraph::new(text).style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(paragraph, area);
}

/// Render the scrollable output pane into `area`.
///
/// Applies `scroll_offset` to show older lines when the user has scrolled up.
/// Uses `compute_render_slice` to select the correct logical-line window,
/// accounting for text wrapping so the last lines are never clipped off screen.
///
/// Records `area` into `state.output.output_area` so the mouse event handler can
/// check whether a wheel event falls within the output zone. Splits the area to
/// reserve one column for the scroll-position indicator, then delegates to
/// `render_scroll_indicator` to draw the track and marker.
///
/// When the content-area width changes between frames (e.g. agent panel
/// opens/closes), recalculates `scroll_offset` to preserve the user's visual
/// position after text reflows. Updates `last_render_width` every frame.
pub(crate) fn render_output(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    state.output.panel_areas.output_area.set(area);

    let (content_area, scrollbar_area) = split_output_area(area);
    let visible = content_area.height as usize;
    let width = content_area.width as usize;

    maybe_recalculate_scroll_for_resize(state, width);
    render_output_content(
        frame,
        OutputContentRender {
            state,
            content_area,
            visible,
            width,
        },
    );
    render_scroll_indicator(frame, state, scrollbar_area);
}

/// Recalculate scroll offset when the content-area width has changed between frames.
///
/// No-ops when the width has not changed. When it has changed, recomputes
/// `scroll_offset` at the new width so the user's visual anchor is preserved
/// after text reflows, then updates `last_render_width`.
fn maybe_recalculate_scroll_for_resize(state: &TuiDisplayState, width: usize) {
    let old_width = state.output.last_render_width.get();
    if old_width == width {
        return;
    }
    let old_offset = state.output.scroll_offset.get();
    if old_offset.inner() > 0 {
        let new_offset = recalculate_scroll_for_width_change(
            &state.output.lines,
            old_offset.inner(),
            WidthChange {
                old: old_width,
                new: width,
            },
        );
        let total_rows: usize = state
            .output
            .lines
            .iter()
            .map(|l| line_display_rows(&rendered_line_text(l), Count::of(width)).inner())
            .sum();
        let clamped = new_offset.min(total_rows.saturating_sub(1));
        if clamped != old_offset.inner() {
            tracing::info!(
                old_width,
                new_width = width,
                old_offset = old_offset.inner(),
                recalculated_offset = new_offset,
                clamped_offset = clamped,
                did_clamp = clamped != new_offset,
                "tui.render.primary_feed.resize_scroll_adjusted"
            );
        }
        state.output.scroll_offset.set(ScrollOffset::of(clamped));
    }
    state.output.last_render_width.set(width);
}

/// Render the output paragraph and optional selection overlay into `content_area`.
struct OutputContentRender<'a> {
    state: &'a TuiDisplayState,
    content_area: Rect,
    visible: usize,
    width: usize,
}

fn render_output_content(frame: &mut Frame, render: OutputContentRender<'_>) {
    let render_slice = compute_render_slice(
        RenderSliceInput::builder()
            .lines(&render.state.output.lines)
            .visible_rows(Count::of(render.visible))
            .scroll_offset(render.state.output.scroll_offset.get())
            .content_width(Count::of(render.width))
            .build(),
    );

    // Calculate how many display rows the content actually occupies (accounting for text wrapping)
    let content_display_rows: usize = render.state.output.lines
        [render_slice.start..render_slice.end]
        .iter()
        .map(|line| {
            let rendered = rendered_line_text(line);
            line_display_rows(&rendered, Count::of(render.width)).inner()
        })
        .sum::<usize>()
        .saturating_sub(render_slice.para_scroll as usize);

    // Add blank padding FIRST, then content (pushes content to bottom)
    let blank_count = render.visible.saturating_sub(content_display_rows);
    let mut lines: Vec<Line> = (0..blank_count).map(|_| Line::from("")).collect();
    lines.extend(
        render.state.output.lines[render_slice.start..render_slice.end]
            .iter()
            .map(output_line_to_ratatui),
    );

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((render_slice.para_scroll, 0));
    frame.render_widget(paragraph, render.content_area);

    if let Some(sel) = &render.state.output.selection {
        apply_selection_overlay(
            frame,
            SelectionRenderContext {
                selection: *sel,
                content_area: render.content_area,
            },
        );
    }
}

/// Render the scroll-position indicator for the main output pane.
///
/// Delegates to `render_scroll_indicator_for` using the main output line count
/// and scroll offset. Callers: `render_output`.
fn render_scroll_indicator(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    render_scroll_indicator_for(
        frame,
        ScrollIndicatorRenderContext {
            area,
            scroll: ScrollRenderContext::builder()
                .total_lines(state.output.lines.len())
                .visible_lines(area.height as usize)
                .scroll_offset(state.output.scroll_offset.get().inner())
                .indicator_height(area.height as usize)
                .build(),
        },
    );
}

/// Render a vertical scroll-position indicator given total lines and scroll offset.
///
/// Draws a one-column track of `│` characters in `SCROLLBAR_TRACK_COLOR`.
/// When scrollable content exists, overlays a single `█` marker at the row
/// computed by `scroll_marker_row` in `SCROLLBAR_MARKER_COLOR`. No-ops when
/// `area` has zero width or height.
/// Callers: `render_scroll_indicator`, `render_ask_panel`.
pub(crate) fn render_scroll_indicator_for(
    frame: &mut Frame,
    context: ScrollIndicatorRenderContext,
) {
    if context.area.height == 0 || context.area.width == 0 {
        return;
    }
    let height = context.area.height as usize;
    let marker = scroll_marker_row(context.scroll);

    let lines: Vec<Line> = (0..height)
        .map(|row| {
            let is_marker = bool::from(marker.visible) && row == marker.row.inner();
            let (ch, color) = if is_marker {
                ('█', SCROLLBAR_MARKER_COLOR)
            } else {
                ('│', SCROLLBAR_TRACK_COLOR)
            };
            Line::from(Span::styled(ch.to_string(), Style::default().fg(color)))
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)), context.area);
}

/// Convert an `OutputLine` to a ratatui `Line`, prepending a dimmed response prefix
/// when `output_line.header` has a timestamp or model prefix set.
///
/// `SelfFeedback` lines are rendered with dim+italic styling so sub-agent monologue
/// is visually distinct. `ToolCall` lines use dimmed styling. `Error` lines use
/// red+bold. `UserInput` lines apply `USER_INPUT_BG` to both spans. `Plain` lines
/// are rendered with no additional styling. The prefix span uses `Modifier::DIM`
/// on all variants so it visually recedes behind the content.
pub(crate) fn output_line_to_ratatui(output_line: &OutputLine) -> Line<'_> {
    let content_span = line_content_span(output_line);
    let prefix = format_response_prefix(&output_line.header);
    if prefix.is_empty() {
        Line::from(content_span)
    } else {
        Line::from(vec![
            line_prefix_span(output_line, prefix.to_string()),
            content_span,
        ])
    }
}

/// Apply a reversed-video selection highlight to the cells covered by `sel`.
///
/// After the output `Paragraph` is rendered, this function overlays `REVERSED`
/// style (inverted fg/bg) on every terminal cell that falls within the selection
/// range. Cells outside `content_area` are skipped. The selection is purely
/// screen-coordinate based - no line-boundary knowledge is required here.
///
/// No-ops when `content_area` has zero width or zero height - there is nothing
/// to highlight in a degenerate area.
///
/// Callers: `render_output` (when `state.output.selection.is_some()`).
pub(crate) fn apply_selection_overlay(frame: &mut Frame, ctx: SelectionRenderContext) {
    let content_area = ctx.content_area;
    if content_area.width == 0 || content_area.height == 0 {
        return;
    }
    let bounds = normalize_selection(&ctx.selection, content_area);
    let ca_x_end = content_area.x + content_area.width;
    let ca_y_end = content_area.y + content_area.height;
    let buf = frame.buffer_mut();
    for row in bounds.y_start..=bounds.y_end {
        if row < content_area.y || row >= ca_y_end {
            continue;
        }
        let col_from = if row == bounds.y_start {
            bounds.x_start
        } else {
            content_area.x
        };
        let col_to = if row == bounds.y_end {
            bounds.x_end.saturating_add(1)
        } else {
            ca_x_end
        };
        for col in col_from..col_to.min(ca_x_end) {
            if let Some(cell) = buf.cell_mut(Position::new(col, row)) {
                let style = cell.style().bg(SELECTION_BG).fg(Color::White);
                cell.set_style(style);
            }
        }
    }
}

fn line_content_span(output_line: &OutputLine) -> Span<'static> {
    let text = output_line.text.as_str().to_owned();
    match &output_line.kind {
        LineKind::Plain | LineKind::System => Span::raw(text),
        kind => Span::styled(text, line_content_style(kind)),
    }
}

fn line_content_style(kind: &LineKind) -> Style {
    match kind {
        LineKind::SelfFeedback => Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC),
        LineKind::ToolCall => Style::default().add_modifier(Modifier::DIM),
        LineKind::Error => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        LineKind::UserInput => Style::default().bg(USER_INPUT_BG),
        LineKind::System => Style::default(),
        LineKind::Plain => Style::default(),
    }
}

fn line_prefix_span(output_line: &OutputLine, prefix: String) -> Span<'static> {
    Span::styled(prefix, line_prefix_style(&output_line.kind))
}

fn line_prefix_style(kind: &LineKind) -> Style {
    match kind {
        LineKind::UserInput => Style::default()
            .bg(USER_INPUT_BG)
            .add_modifier(Modifier::DIM),
        LineKind::Error => Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
        _ => Style::default().add_modifier(Modifier::DIM),
    }
}

/// Recalculate `scroll_offset` after a content-area width change.
///
/// `scroll_offset` is a count of display rows skipped from the bottom. When
/// the render width changes, text reflows and the same display-row count may no
/// longer correspond to the same visual anchor position. This function finds
/// the anchor line by walking backward `old_offset` display rows at `old_width`,
/// then recomputes the display-row count from that anchor to the end at `new_width`.
///
/// Returns `old_offset` unchanged when:
/// - `old_offset` is 0 (user is at the bottom - nothing to preserve), or
/// - either width is 0 (degenerate/uninitialized - no reflow to account for).
///
/// Callers: `render_output` (on every frame where `last_render_width` differs
/// from the current content-area width).
fn recalculate_scroll_for_width_change(
    lines: &[OutputLine],
    old_offset: usize,
    width_change: WidthChange,
) -> usize {
    let is_no_op = old_offset == 0 || width_change.old == 0 || width_change.new == 0;
    if is_no_op {
        return old_offset;
    }

    // Find the anchor: walk backward at old_width, accumulate display rows until
    // we have counted old_offset rows. The anchor marks the start of the skipped region.
    let mut accumulated = 0usize;
    let mut anchor_idx = lines.len();
    for (i, line) in lines.iter().enumerate().rev() {
        let rows =
            line_display_rows(&rendered_line_text(line), Count::of(width_change.old)).inner();
        if accumulated + rows > old_offset {
            anchor_idx = i + 1;
            break;
        }
        accumulated += rows;
        anchor_idx = i;
        if accumulated >= old_offset {
            break;
        }
    }

    // Recount display rows from the anchor to the end at new_width.
    let new_offset: usize = lines[anchor_idx..]
        .iter()
        .map(|l| line_display_rows(&rendered_line_text(l), Count::of(width_change.new)).inner())
        .sum();

    new_offset
}
