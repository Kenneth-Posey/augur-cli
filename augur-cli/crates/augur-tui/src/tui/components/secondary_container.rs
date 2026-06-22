//! Secondary container rendering: ask panel and agent feed panel.
//!
//! Dispatches based on `state.interaction.panel.secondary_view` and renders either
//! the ask side-channel panel or the live agent feed panel into the provided
//! `area`. Both panels share the same layout scheme:
//!
//! - Content area: full height minus 4 bottom rows (scrollable output).
//! - Blank row.
//! - Agent selector row.
//! - Blank row.
//! - Thinking row.
//!
//! Agent feed title format:
//! - Active task + model: `"⠋ [ task-name | model-name ]"` (spinner before label).
//! - Active task, no model: `"⠋ [ task-name ]"`.
//! - No active task: `"[ tasks ]"`.
//!
//! The agent feed shows the selected transcript plus selector/thinking rows at the
//! bottom of the panel.

use crate::domain::tui_display_state::TuiDisplayState;
use crate::domain::tui_render::{
    RenderSliceInput, compute_render_slice, line_display_rows, rendered_line_text,
};
use crate::domain::tui_state::{InputFocus, SecondaryView};
use crate::tui::components::primary_feed::{
    BRAILLE_FRAMES, output_line_to_ratatui, render_scroll_indicator_for, split_output_area,
};
use augur_domain::domain::newtypes::{Count, NumericNewtype, ScrollOffset};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

#[derive(Clone, Copy)]
struct AskViewProps {
    focused: bool,
    area: Rect,
}

#[derive(Clone, Copy, bon::Builder)]
struct OutputPaneRender<'a> {
    lines: &'a [crate::domain::tui_state::OutputLine],
    scroll_offset: ScrollOffset,
    area: Rect,
}

/// Render the secondary container (ask panel or agent feed) into `area`.
///
/// Dispatches to `render_ask_view` when `secondary_view` is `Some(Ask)` and to
/// `render_agent_feed_view` when `secondary_view` is `Some(AgentFeed)`. No-ops
/// when `secondary_view` is `None` or `area.height < 2`.
///
/// Critical: clears `secondary_panel_area` to `Rect::default()` when no secondary
/// view is active to prevent stale coordinates from intercepting main panel mouse events.
pub(crate) fn render_secondary_container(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    // Only set the area if a secondary view is active and has sufficient height
    let should_render = area.height >= 2 && state.interaction.panel.secondary_view.is_some();

    if should_render {
        state.output.panel_areas.secondary_panel_area.set(area);
    } else {
        state
            .output
            .panel_areas
            .secondary_panel_area
            .set(Rect::default());
    }

    if area.height < 2 {
        return;
    }

    match &state.interaction.panel.secondary_view {
        None => {}
        Some(SecondaryView::Ask) => {
            let focused = is_ask_focused(state);
            render_ask_view(frame, state, AskViewProps { focused, area });
        }
        Some(SecondaryView::AgentFeed) => render_agent_feed_view(frame, state, area),
    }
}

/// True when the ask panel input has keyboard focus.
fn is_ask_focused(state: &TuiDisplayState) -> bool {
    state.interaction.panel.input_focus == InputFocus::Ask
}

/// Render the ask side-channel panel into `area`.
///
/// Layout matches the agent panel pattern:
/// - Content area: full height minus 2 bottom rows.
/// - Blank row at `y = area.height - 2`.
/// - Bottom row: `"[ {model} ] ⠋"` when thinking, `"[ {model} ]"` when idle,
///   with `"[ ask ]"` as the fallback label when no model is known.
///
/// The title style is cyan when the ask input has focus, dimmed otherwise.
/// No dedicated spinner row is carved out of the content area.
///
/// No-ops when `ask_panel` is `None`.
fn render_ask_view(frame: &mut Frame, state: &TuiDisplayState, props: AskViewProps) {
    let panel = match &state.interaction.panel.ask_panel {
        Some(p) => p,
        None => return,
    };

    // Reserve 2 rows at the bottom: 1 blank + 1 for title.
    let bottom_reserved = 2u16;
    let output_area = Rect {
        height: props.area.height.saturating_sub(bottom_reserved),
        ..props.area
    };

    render_output_pane(
        frame,
        OutputPaneRender::builder()
            .lines(&panel.output)
            .scroll_offset(panel.scroll)
            .area(output_area)
            .build(),
    );

    let blank_row_area = Rect {
        y: props.area.y + props.area.height.saturating_sub(2),
        height: 1,
        ..props.area
    };
    frame.render_widget(Paragraph::new(""), blank_row_area);

    let title_area = Rect {
        y: props.area.y + props.area.height.saturating_sub(1),
        height: 1,
        ..props.area
    };
    let inline_spinner = bool::from(panel.thinking).then(|| spinner_char(state));
    let model = &state.interaction.panel.agent_feed.current_agent_model;
    let title_text = build_ask_title_text(model, inline_spinner);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            &title_text,
            ask_title_style(props.focused),
        ))),
        title_area,
    );
}

/// Number of bottom rows reserved for the agent feed chrome.
const AGENT_FEED_BOTTOM_ROWS: u16 = 4;

/// Compute the agent feed panel title text.
///
/// Returns a formatted title string based on the active task and model:
/// - When a spinner is present and a task name is known:
///   - With model: `"⠋ [ task-name | model-name ]"`
///   - Without model: `"⠋ [ task-name ]"`
/// - When no active task: `"[ tasks ]"` (spinner and model name are omitted).
fn build_agent_feed_title_text(
    task: &Option<augur_domain::domain::string_newtypes::TaskName>,
    model: &Option<augur_domain::domain::string_newtypes::ModelLabel>,
    spinner: Option<char>,
) -> String {
    let Some(ch) = spinner else {
        return "[ tasks ]".to_string();
    };
    match (task.as_ref(), model.as_ref()) {
        (Some(t), Some(m)) => format!("{ch} [ {} | {} ]", t, m),
        (Some(t), None) => format!("{ch} [ {} ]", t),
        (None, _) => "[ tasks ]".to_string(),
    }
}

fn build_agent_feed_selector_text(
    feed: &crate::domain::tui_state::AgentFeedState,
) -> (String, bool, bool) {
    let Some(selected_index) = feed.selected_feed else {
        return ("[ tasks ]".to_string(), false, false);
    };
    let Some(selected_feed) = feed.feeds.get(selected_index) else {
        return ("[ tasks ]".to_string(), false, false);
    };
    let label = match &selected_feed.feed_id {
        augur_domain::domain::types::FeedId::Agent(id) => id.to_string(),
        augur_domain::domain::types::FeedId::AskPanel => "ask".to_string(),
        augur_domain::domain::types::FeedId::MainConversation => "main".to_string(),
    };
    let has_multiple = feed.feeds.len() >= 2;
    let can_prev = has_multiple && selected_index > 0;
    let can_next = has_multiple && selected_index + 1 < feed.feeds.len();
    (label, can_prev, can_next)
}

/// Compute the ask panel bottom title text.
///
/// Returns the model name formatted as `"[ {model} ]"` when a model is known,
/// or `"[ ask ]"` when no model is set. When `spinner` is `Some(ch)` (i.e.,
/// `ask_panel.thinking` is true), appends a space and the spinner character:
/// `"[ model ] ⠋"`.
fn build_ask_title_text(
    model: &Option<augur_domain::domain::string_newtypes::ModelLabel>,
    spinner: Option<char>,
) -> String {
    let base = if let Some(model) = model {
        format!("[ {} ]", model)
    } else {
        "[ ask ]".to_string()
    };
    match spinner {
        Some(ch) => format!("{base} {ch}"),
        None => base,
    }
}

/// Render the agent feed panel into `area`.
///
/// Layout (example with height=20):
/// - Rows 0-15 (16 rows): scrollable output content area.
/// - Row 16: blank separator row.
/// - Row 17: selector row for the currently selected background agent.
/// - Row 18: blank separator row.
/// - Row 19: thinking row - cyan, format `"⠋ [ task | model ]"` when active,
///   `"[ tasks ]"` when idle. Spinner appears before the label.
fn render_agent_feed_view(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    let feed = &state.interaction.panel.agent_feed;

    // Reserve 4 rows at the bottom: blank, selector, blank, thinking.
    let bottom_reserved = AGENT_FEED_BOTTOM_ROWS;
    let output_area = Rect {
        height: area.height.saturating_sub(bottom_reserved),
        ..area
    };

    let display_lines = build_agent_feed_display_lines(feed);
    render_output_pane(
        frame,
        OutputPaneRender::builder()
            .lines(&display_lines)
            .scroll_offset(feed.scroll)
            .area(output_area)
            .build(),
    );

    render_agent_feed_chrome(frame, AgentFeedChromeRender { state, area, feed });
}

// ── Style helpers ─────────────────────────────────────────────────────────────

/// Return the title style for the ask panel: cyan when focused, dimmed otherwise.
fn ask_title_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

/// Return the current spinner character from the shared Braille frame array.
fn spinner_char(state: &TuiDisplayState) -> char {
    let frame_idx = (state.agent.thinking.spinner_tick as usize) % BRAILLE_FRAMES.len();
    BRAILLE_FRAMES[frame_idx]
}

// ── Render helpers ────────────────────────────────────────────────────────────

/// Build the display lines for the agent feed panel.
///
/// During an active task, StatusLine and ToolEvent chunks accumulate in
/// `buffers` and are only flushed to `output` at task boundaries. Appending
/// them here ensures live content is visible before the flush occurs.
fn build_agent_feed_display_lines(
    feed: &crate::domain::tui_state::AgentFeedState,
) -> Vec<crate::domain::tui_state::OutputLine> {
    if feed.buffers.pending_tool_event.is_none() && feed.buffers.pending_status_message.is_none() {
        return feed.output.clone();
    }
    feed.output
        .iter()
        .cloned()
        .chain(feed.buffers.pending_status_message.iter().cloned())
        .chain(feed.buffers.pending_tool_event.iter().cloned())
        .collect()
}

/// Render the visual bottom chrome (blank row, selector row, blank row, and thinking row)
/// for the agent feed panel.
struct AgentFeedChromeRender<'a> {
    state: &'a TuiDisplayState,
    area: Rect,
    feed: &'a crate::domain::tui_state::AgentFeedState,
}

fn render_agent_feed_chrome(frame: &mut Frame, render: AgentFeedChromeRender<'_>) {
    // Blank row above selector.
    let blank_row_area = Rect {
        y: render.area.y + render.area.height.saturating_sub(4),
        height: 1,
        ..render.area
    };
    frame.render_widget(Paragraph::new(""), blank_row_area);

    let selector_area = Rect {
        y: render.area.y + render.area.height.saturating_sub(3),
        height: 1,
        ..render.area
    };
    let (label, can_prev, can_next) = build_agent_feed_selector_text(render.feed);
    let selector_line = if render.feed.feeds.len() >= 2 {
        let left_style = if can_prev {
            Style::default().fg(Color::White)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        };
        let right_style = if can_next {
            Style::default().fg(Color::White)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        };
        Line::from(vec![
            Span::styled("‹", left_style),
            Span::raw(" "),
            Span::styled(label, Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled("›", right_style),
        ])
    } else {
        Line::from(vec![Span::styled(label, Style::default().fg(Color::Cyan))])
    };
    frame.render_widget(Paragraph::new(selector_line), selector_area);

    let thinking_blank_area = Rect {
        y: render.area.y + render.area.height.saturating_sub(2),
        height: 1,
        ..render.area
    };
    frame.render_widget(Paragraph::new(""), thinking_blank_area);

    let thinking_area = Rect {
        y: render.area.y + render.area.height.saturating_sub(1),
        height: 1,
        ..render.area
    };
    let inline_spinner = render
        .feed
        .active_task
        .is_some()
        .then(|| spinner_char(render.state));
    let title_text = build_agent_feed_title_text(
        &render.feed.active_task,
        &render.feed.current_agent_model,
        inline_spinner,
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            &title_text,
            Style::default().fg(Color::Cyan),
        ))),
        thinking_area,
    );
}

///
/// Splits `output_area` into content and scrollbar columns, computes the visible
/// slice from `lines` and `scroll_offset`, pads blank lines at the top so content
/// is bottom-aligned, and draws the scroll indicator on the right edge.
fn render_output_pane(frame: &mut Frame, render: OutputPaneRender<'_>) {
    let (content_area, scrollbar_area) = split_output_area(render.area);
    let visible = content_area.height as usize;
    let width = content_area.width as usize;
    let total_display_rows = total_display_rows_for_lines(render.lines, width);
    let max_offset = total_display_rows.saturating_sub(visible);
    let effective_scroll = ScrollOffset::of(render.scroll_offset.inner().min(max_offset));
    let render_slice = compute_render_slice(
        RenderSliceInput::builder()
            .lines(render.lines)
            .visible_rows(Count::of(visible))
            .scroll_offset(effective_scroll)
            .content_width(Count::of(width))
            .build(),
    );

    let content_display_rows: usize = render.lines[render_slice.start..render_slice.end]
        .iter()
        .map(|line| line_display_rows(&rendered_line_text(line), Count::of(width)).inner())
        .sum::<usize>()
        .saturating_sub(render_slice.para_scroll as usize);
    let blank_count = visible.saturating_sub(content_display_rows);
    let mut all_lines: Vec<Line> = (0..blank_count).map(|_| Line::from("")).collect();
    all_lines.extend(
        render.lines[render_slice.start..render_slice.end]
            .iter()
            .map(output_line_to_ratatui),
    );

    frame.render_widget(
        Paragraph::new(Text::from(all_lines))
            .wrap(Wrap { trim: false })
            .scroll((render_slice.para_scroll, 0)),
        content_area,
    );
    render_scroll_indicator_for(
        frame,
        super::primary_feed::ScrollIndicatorRenderContext {
            area: scrollbar_area,
            scroll: super::primary_feed_utils::ScrollRenderContext::builder()
                .total_lines(total_display_rows)
                .visible_lines(content_area.height as usize)
                .scroll_offset(effective_scroll.inner())
                .indicator_height(scrollbar_area.height as usize)
                .build(),
        },
    );
}

fn total_display_rows_for_lines(
    lines: &[crate::domain::tui_state::OutputLine],
    width: usize,
) -> usize {
    lines
        .iter()
        .map(|line| line_display_rows(&rendered_line_text(line), Count::of(width)).inner())
        .sum()
}
