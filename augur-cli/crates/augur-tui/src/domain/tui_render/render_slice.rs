//! Render-slice calculation and output-line text formatting helpers.

use crate::domain::tui_state::{LineHeader, OutputLine};
use augur_domain::domain::newtypes::{Count, NumericNewtype, ScrollOffset};
use augur_domain::domain::string_newtypes::OutputText;
use chrono::{DateTime, Local};
use unicode_width::UnicodeWidthChar;

/// Input contract for computing the visible output slice.
///
/// The `visible_rows` and `content_width` counts describe the wrapped paragraph
/// viewport. `scroll_offset` is the number of display rows to skip from the bottom.
#[derive(Clone, Copy, bon::Builder)]
pub struct RenderSliceInput<'a> {
    pub(crate) lines: &'a [OutputLine],
    pub(crate) visible_rows: Count,
    pub(crate) scroll_offset: ScrollOffset,
    pub(crate) content_width: Count,
}

/// Computed render window for the output paragraph.
///
/// `start..end` is the logical-line slice to render. `para_scroll` is the
/// wrapped-row offset applied to the first rendered line via `Paragraph::scroll`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, bon::Builder)]
pub struct RenderSlice {
    pub start: usize,
    pub end: usize,
    pub para_scroll: u16,
}

/// Compute the wrapped output slice that keeps the last visible rows on screen.
pub fn compute_render_slice(input: RenderSliceInput<'_>) -> RenderSlice {
    let content_end = trim_trailing_padding_lines(input.lines);
    let visible_lines = &input.lines[..content_end];
    let bottom_cutoff = skip_display_rows_from_bottom(
        visible_lines,
        input.scroll_offset.inner(),
        input.content_width.inner(),
    );
    let (start, para_scroll) = fill_from_bottom(
        &visible_lines[..bottom_cutoff],
        input.visible_rows.inner(),
        input.content_width.inner(),
    );
    RenderSlice::builder()
        .start(start)
        .end(bottom_cutoff)
        .para_scroll(para_scroll)
        .build()
}

/// Return the exclusive end index after removing trailing visual padding rows.
///
/// Conversation turns intentionally append blank separator lines. These should
/// not become the anchor for bottom-follow rendering; users expect the newest
/// timestamped/content row to remain visible at the bottom.
fn trim_trailing_padding_lines(lines: &[OutputLine]) -> usize {
    let mut end = lines.len();
    while end > 0 {
        if is_visually_empty_line(&lines[end - 1]) {
            end -= 1;
            continue;
        }
        break;
    }
    end
}

fn is_visually_empty_line(line: &OutputLine) -> bool {
    rendered_line_text(line).trim().is_empty()
}

/// Walk backward from the end of `lines`, excluding lines until `skip_rows`
/// display rows have been accumulated. Returns the exclusive-end index
/// (bottom_cutoff) for the visible region: `lines[..result]`.
///
/// Lines that fit entirely within the remaining skip budget are excluded. A
/// line whose display-row count would exceed the remaining budget is kept
/// visible (its boundary row will be handled by `fill_from_bottom` via
/// `para_scroll`). Returns `lines.len()` when `skip_rows` is zero.
fn skip_display_rows_from_bottom(lines: &[OutputLine], skip_rows: usize, width: usize) -> usize {
    if skip_rows == 0 {
        return lines.len();
    }
    let mut accumulated = 0usize;
    let mut cutoff = lines.len();
    for (i, line) in lines.iter().enumerate().rev() {
        let rows = line_display_rows(&rendered_line_text(line), Count::new(width)).inner();
        if accumulated + rows > skip_rows {
            cutoff = i + 1;
            break;
        }
        accumulated += rows;
        cutoff = i;
        if accumulated >= skip_rows {
            break;
        }
    }
    cutoff
}

/// Estimate the number of display rows a logical output line occupies for a
/// given content width.
///
/// Uses a greedy word-wrap algorithm that matches ratatui's `Wrap { trim: false }`
/// behaviour: a word that does not fit on the current row wraps to the next;
/// words longer than the row width are character-broken across rows.
///
/// Pure character-count division underestimates the row count for spaced text
/// because word boundaries leave unused column space, causing `para_scroll` to
/// be too small and the end of a long streaming message to fall below the
/// viewport. This implementation correctly accounts for that overhead.
pub fn line_display_rows(text: &OutputText, width: Count) -> Count {
    let w = width.inner();
    if should_force_single_row(text, w) {
        return Count::new(1);
    }
    let s = &**text;
    let mut state = WordWrapState::new(w);
    let mut word_cols = 0usize;

    for ch in s.chars() {
        handle_wrap_char(&mut state, ch, &mut word_cols);
    }
    flush_pending_word(&mut state, &mut word_cols);
    Count::new(state.rows)
}

fn should_force_single_row(text: &OutputText, width: usize) -> bool {
    width == 0 || text.is_empty()
}

fn handle_wrap_char(state: &mut WordWrapState, ch: char, word_cols: &mut usize) {
    match classify_wrap_char(ch) {
        WrapChar::Newline => {
            flush_pending_word(state, word_cols);
            state.newline();
        }
        WrapChar::Space => {
            flush_pending_word(state, word_cols);
            state.add_space();
        }
        WrapChar::Glyph(width) => {
            *word_cols += width;
        }
    }
}

fn flush_pending_word(state: &mut WordWrapState, word_cols: &mut usize) {
    if *word_cols == 0 {
        return;
    }
    state.place_word(*word_cols);
    *word_cols = 0;
}

enum WrapChar {
    Newline,
    Space,
    Glyph(usize),
}

fn classify_wrap_char(ch: char) -> WrapChar {
    match ch {
        '\n' => WrapChar::Newline,
        ' ' => WrapChar::Space,
        _ => WrapChar::Glyph(ch.width().unwrap_or(0)),
    }
}

/// Mutable cursor state for the greedy word-wrap row estimator.
/// All measurements are in display columns, not char count.
struct WordWrapState {
    rows: usize,
    col: usize,
    width: usize,
}

impl WordWrapState {
    fn new(width: usize) -> Self {
        Self {
            rows: 1,
            col: 0,
            width,
        }
    }

    /// Advance past a hard newline.
    fn newline(&mut self) {
        self.rows += 1;
        self.col = 0;
    }

    /// Advance past a single space character (trim: false - kept on new row).
    fn add_space(&mut self) {
        if self.col < self.width {
            self.col += 1;
        } else {
            self.rows += 1;
            self.col = 1;
        }
    }

    /// Place a word of `word_cols` display columns: wrap before it when it does not fit
    /// on the current row, then character-break across additional rows if
    /// the word is longer than the row width.
    fn place_word(&mut self, word_len: usize) {
        if self.col > 0 && self.col + word_len > self.width {
            self.rows += 1;
            self.col = 0;
        }
        let mut remaining = word_len;
        while self.col + remaining > self.width {
            self.rows += 1;
            let placed = self.width - self.col;
            remaining -= placed;
            self.col = 0;
        }
        self.col += remaining;
    }
}

/// Build the full rendered text of an output line as it appears in the terminal.
pub fn rendered_line_text(line: &OutputLine) -> OutputText {
    let prefix = format_response_prefix(&line.header);
    if prefix.is_empty() {
        line.text.clone()
    } else {
        OutputText::from(format!("{}{}", prefix, line.text))
    }
}

/// Format a `LineHeader` as a response prefix string.
pub fn format_response_prefix(header: &LineHeader) -> OutputText {
    let ts_part = header.timestamp.map(|ts| {
        let dt: DateTime<Local> = DateTime::from_timestamp_millis(ts.inner() as i64)
            .map(|utc| utc.with_timezone(&Local))
            .unwrap_or_else(Local::now);
        format!("[{}] ", dt.format("%H:%M:%S"))
    });
    match (&ts_part, &header.model_prefix) {
        (Some(ts), Some(model)) => OutputText::from(format!("{}{} > ", ts, model)),
        (Some(ts), None) => OutputText::from(ts.clone()),
        (None, Some(model)) => OutputText::from(format!("{} > ", model)),
        (None, None) => OutputText::from(""),
    }
}

/// Walk backwards through `lines`, accumulating display rows until `visible`
/// rows are filled or all lines are consumed.
fn fill_from_bottom(lines: &[OutputLine], visible: usize, content_width: usize) -> (usize, u16) {
    let n = lines.len();
    if n == 0 || visible == 0 {
        return (0, 0);
    }
    let mut need = visible;
    let mut start = n;
    let mut para_scroll = 0u16;
    for i in (0..n).rev() {
        let rendered = rendered_line_text(&lines[i]);
        let rows = line_display_rows(&rendered, Count::new(content_width)).inner();
        let Some(updated_need) = remaining_need_after_full_line(need, rows) else {
            para_scroll = compute_partial_line_scroll(rows, need);
            start = i;
            break;
        };
        need = updated_need;
        start = i;
        if need == 0 {
            break;
        }
    }

    (start, para_scroll)
}

fn remaining_need_after_full_line(need: usize, rows: usize) -> Option<usize> {
    if rows > need {
        return None;
    }
    Some(need.saturating_sub(rows))
}

fn compute_partial_line_scroll(rows: usize, need: usize) -> u16 {
    (rows - need) as u16
}
