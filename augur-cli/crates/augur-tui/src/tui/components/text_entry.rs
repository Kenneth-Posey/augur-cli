//! Text entry and completion hint rendering functions.

use crate::domain::tui_display_state::TuiDisplayState;
use crate::domain::tui_state::InputFocus;
use augur_domain::domain::newtypes::Count;
use augur_domain::domain::types::{CommandDef, FileCompletion, ModelOption};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};

#[derive(Clone, Copy)]
struct HintRenderState {
    selected: Option<usize>,
    area: Rect,
}

/// Discriminant describing which completion hint list is currently active.
///
/// Produced by `active_hint_kind` and consumed by `active_hint_count` and
/// `render_active_hints` via a `match` to avoid duplicating the priority logic.
enum HintKind {
    ThinkingMode,
    Commands,
    Files,
    Models,
}

/// Determine which hint list takes priority given the current `AppState`.
///
/// Priority order: thinking-mode picker → command completions → file completions → model picker.
/// Each branch uses an early return so the function body contains no else-if chains.
fn active_hint_kind(state: &TuiDisplayState) -> HintKind {
    let thinking_mode_open = state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id
        .is_some();
    if thinking_mode_open {
        return HintKind::ThinkingMode;
    }
    if !state.prompt.completions.commands.is_empty() {
        return HintKind::Commands;
    }
    if !state.prompt.completions.files.is_empty() {
        return HintKind::Files;
    }
    HintKind::Models
}

/// Width (chars) reserved for the usage column in the completion list.
///
/// Provides consistent padding between the usage and description columns when
/// multiple completions are rendered. Matches the column width used by the
/// registry's help text formatter.
const COMPLETION_USAGE_WIDTH: usize = 22;

/// Return the number of completion hint items currently active.
///
/// Used by both `render_chat_layout` and `render_conversation_container` to pass a
/// consistent hint count into `compute_layout`.
pub(crate) fn active_hint_count(state: &TuiDisplayState) -> Count {
    match active_hint_kind(state) {
        HintKind::ThinkingMode => {
            Count::of(augur_domain::domain::thinking_mode::ReasoningEffort::options().len())
        }
        HintKind::Commands => Count::of(state.prompt.completions.commands.len()),
        HintKind::Files => Count::of(state.prompt.completions.files.len()),
        HintKind::Models => Count::of(state.prompt.completions.model_picker.items.len()),
    }
}

/// Render whichever completion hint list is active into `area`.
///
/// Dispatches to `render_thinking_mode_hints`, `render_command_hints`,
/// `render_file_hints`, or `render_model_hints` based on which list is active.
/// Thinking mode takes priority when its picker is open.
pub(crate) fn render_active_hints(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    match active_hint_kind(state) {
        HintKind::ThinkingMode => render_thinking_mode_hints(
            frame,
            HintRenderState {
                selected: state.prompt.completions.model_picker.thinking_mode.selected,
                area,
            },
        ),
        HintKind::Commands => render_command_hints(
            frame,
            &state.prompt.completions.commands,
            HintRenderState {
                selected: state.prompt.completions.command_selected,
                area,
            },
        ),
        HintKind::Files => render_file_hints(
            frame,
            &state.prompt.completions.files,
            HintRenderState {
                selected: state.prompt.completions.file_selected,
                area,
            },
        ),
        HintKind::Models => render_model_hints(
            frame,
            &state.prompt.completions.model_picker.items,
            HintRenderState {
                selected: state.prompt.completions.model_picker.selected,
                area,
            },
        ),
    }
}

/// Render the dynamic input area with a reversed-character cursor at the current byte offset.
///
/// Splits the buffer into three spans: text before the cursor, the character AT
/// the cursor rendered with `Modifier::REVERSED` (or a reversed space when the
/// cursor is at the end), and text after the cursor. The cursor position is a
/// byte offset kept at a valid UTF-8 char boundary by `apply_key`. Wrapping is
/// enabled so the display height matches `compute_input_height`.
pub(crate) fn render_input(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    let buf = &state.prompt.buffer;
    let cursor = state.prompt.cursor;
    let (before, cursor_char, after) = if cursor < buf.len() {
        let end = next_char_boundary(buf, cursor);
        (&buf[..cursor], &buf[cursor..end], &buf[end..])
    } else {
        (buf.as_str(), " ", "")
    };
    let ask_focused = state.interaction.panel.input_focus == InputFocus::Ask;
    let line = if ask_focused {
        Line::from(vec![
            Span::styled("[ask] ❯ ", Style::default().fg(Color::Cyan)),
            Span::raw(before.to_owned()),
            Span::styled(
                cursor_char.to_owned(),
                Style::default().add_modifier(Modifier::REVERSED),
            ),
            Span::raw(after.to_owned()),
        ])
    } else {
        Line::from(vec![
            Span::raw(format!("❯ {}", before)),
            Span::styled(
                cursor_char.to_owned(),
                Style::default().add_modifier(Modifier::REVERSED),
            ),
            Span::raw(after.to_owned()),
        ])
    };
    let paragraph = Paragraph::new(Text::from(vec![line])).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn next_char_boundary(s: &str, byte_pos: usize) -> usize {
    let mut pos = byte_pos + 1;
    while pos < s.len() && !s.is_char_boundary(pos) {
        pos += 1;
    }
    pos.min(s.len())
}

/// Render the command completion list above the input area.
///
/// Each `CommandDef` is formatted as `"❯ /usage   description"` for the selected
/// item and `"  /usage   description"` for all others. The `❯ ` marker acts as
/// the cursor that the user moves with Up/Down arrows. Non-selected rows are
/// rendered with `Modifier::DIM` so the selected row stands out at normal
/// brightness. Renders nothing when `completions` is empty (the allocated `area`
/// has zero height when inactive).
fn render_command_hints(frame: &mut Frame, completions: &[CommandDef], render: HintRenderState) {
    if completions.is_empty() {
        return;
    }
    let styled_lines: Vec<Line> = completions
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let is_selected = render.selected == Some(i);
            let text = format_completion_line(cmd, is_selected);
            let style = if is_selected {
                Style::default()
            } else {
                Style::default().add_modifier(Modifier::DIM)
            };
            Line::from(Span::styled(text, style))
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(styled_lines)), render.area);
}

/// Format a single completion entry as a display line.
///
/// Selected entries are prefixed with `"❯ "` and unselected with `"  "`.
/// The usage column is padded to `COMPLETION_USAGE_WIDTH` chars so the
/// description column aligns vertically across all entries in the list.
fn format_completion_line(cmd: &CommandDef, is_selected: bool) -> String {
    let marker = if is_selected { "❯ " } else { "  " };
    format!(
        "{}{:<width$}{}",
        marker,
        cmd.usage,
        cmd.description,
        width = COMPLETION_USAGE_WIDTH
    )
}

/// Render file path completions in the hint zone using the same visual style as
/// `render_command_hints`.
///
/// Each row shows the `display_name` left-justified, prefixed with the selection
/// marker. The selected row is rendered at normal brightness; unselected rows are
/// dimmed. Renders nothing when `files` is empty.
///
/// Consumers: `render_active_hints` when file completions are active.
fn render_file_hints(frame: &mut Frame, files: &[FileCompletion], render: HintRenderState) {
    if files.is_empty() {
        return;
    }
    let styled_lines: Vec<Line> = files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let is_selected = render.selected == Some(i);
            let marker = if is_selected { "❯ " } else { "  " };
            let text = format!("{}{}", marker, f.display_name);
            let style = if is_selected {
                Style::default()
            } else {
                Style::default().add_modifier(Modifier::DIM)
            };
            Line::from(Span::styled(text, style))
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(styled_lines)), render.area);
}

/// Render model picker completions in the hint zone.
///
/// Each row shows `"{marker}{display_name}"`.
/// The selected row renders at normal brightness; unselected rows are dimmed.
/// Renders nothing when `models` is empty (the allocated `area` has zero height
/// when inactive).
///
/// Consumers: `render_active_hints` when model picker completions are active.
fn render_model_hints(frame: &mut Frame, models: &[ModelOption], render: HintRenderState) {
    if models.is_empty() || render.area.height == 0 {
        return;
    }
    let visible_rows = render.area.height as usize;
    let selected_index = render.selected.filter(|&index| index < models.len());
    let first_visible = selected_index
        .map(|index| index.saturating_add(1).saturating_sub(visible_rows))
        .unwrap_or(0)
        .min(models.len().saturating_sub(visible_rows));
    let styled_lines: Vec<Line> = models
        .iter()
        .enumerate()
        .skip(first_visible)
        .take(visible_rows)
        .map(|(i, m)| {
            let is_selected = render.selected == Some(i);
            let marker = if is_selected { "❯ " } else { "  " };
            let text = format!("{}{}", marker, m.display_name);
            let style = if is_selected {
                Style::default()
            } else {
                Style::default().add_modifier(Modifier::DIM)
            };
            Line::from(Span::styled(text, style))
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(styled_lines)), render.area);
}

/// Render thinking mode (reasoning effort) options in the hint zone.
///
/// Displays all five `ReasoningEffort` options using their `display_label()` text.
/// The selected row renders at normal brightness; all other rows are dimmed.
/// When `render.selected` is `None`, all rows render dimmed (no selection).
///
/// Consumers: `render_active_hints` when the thinking mode picker is open.
fn render_thinking_mode_hints(frame: &mut Frame, render: HintRenderState) {
    use augur_domain::domain::thinking_mode::ReasoningEffort;
    if render.area.height == 0 {
        return;
    }
    let options = ReasoningEffort::options();
    let visible_rows = render.area.height as usize;
    let selected_index = render.selected.filter(|&index| index < options.len());
    let first_visible = selected_index
        .map(|index| index.saturating_add(1).saturating_sub(visible_rows))
        .unwrap_or(0)
        .min(options.len().saturating_sub(visible_rows));
    let styled_lines: Vec<Line> = options
        .iter()
        .enumerate()
        .skip(first_visible)
        .take(visible_rows)
        .map(|(i, opt)| {
            let is_selected = render.selected == Some(i);
            let marker = if is_selected { "❯ " } else { "  " };
            let text = format!("{}{}", marker, opt.display_label());
            let style = if is_selected {
                Style::default()
            } else {
                Style::default().add_modifier(Modifier::DIM)
            };
            Line::from(Span::styled(text, style))
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(styled_lines)), render.area);
}
