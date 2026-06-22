//! Footer and status bar rendering: controls row, status bar, context meter.

use crate::domain::tui_display_state::{DisplayConversationMode, TuiDisplayState};
use crate::domain::tui_state::{OutputSelection, SecondaryView, StatusBarData};
use augur_domain::domain::newtypes::UsdCost;
use augur_domain::domain::string_newtypes::{StatusLabel, StringNewtype};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlHint {
    pub key: StatusLabel,
    pub description: StatusLabel,
}

/// Return the keyboard hint label pair for the bottom controls row.
///
/// Priority: secondary view open > plan/guided-plan mode > default.
/// - `Some(Ask)` open: `("ctrl+w", "close ask")`
/// - `Some(AgentFeed)` open: `("ctrl+w", "close tasks")`
/// - Plan mode (no secondary): `("esc", "close plan")`
/// - Default: `("shift+tab", "open ask")`
///
/// Made `pub(crate)` so render tests can verify hint logic independently.
pub fn controls_row_hint(
    secondary: Option<&SecondaryView>,
    mode: &DisplayConversationMode,
) -> ControlHint {
    let (key, description) = match secondary {
        Some(SecondaryView::Ask) => ("ctrl+w", "close ask"),
        Some(SecondaryView::AgentFeed) => ("ctrl+w", "close tasks"),
        None if matches!(
            mode,
            DisplayConversationMode::Plan(_) | DisplayConversationMode::GuidedPlan(_)
        ) =>
        {
            ("esc", "close plan")
        }
        None => ("shift+tab", "open ask"),
    };
    ControlHint {
        key: StatusLabel::new(key),
        description: StatusLabel::new(description),
    }
}

/// Render the keyboard-hint controls row at the bottom of the terminal.
///
/// Shows the hint key in bold followed by the description in dimmed style.
/// The hint pair is selected by `controls_row_hint` based on mode and panel state.
pub(crate) fn render_controls_row(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    if area.height == 0 {
        return;
    }
    let mut hints = vec![controls_row_hint(
        state.interaction.panel.secondary_view.as_ref(),
        &state.interaction.mode,
    )];
    if matches!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::AgentFeed)
    ) && state.interaction.panel.agent_feed.feeds.len() >= 2
    {
        hints.push(ControlHint {
            key: StatusLabel::new("ctrl+o"),
            description: StatusLabel::new("prev agent"),
        });
        hints.push(ControlHint {
            key: StatusLabel::new("ctrl+p"),
            description: StatusLabel::new("next agent"),
        });
    }
    let mut spans = Vec::new();
    for (idx, hint) in hints.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" | "));
        }
        spans.push(Span::styled(
            hint.key.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            hint.description.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

/// Render the status bar with cwd+branch on the left and token totals+model on the right.
///
/// Left: `"{cwd} [{branch}]"` (or just `"{cwd}"` when no git branch is available).
/// Right: `"{in}↑ {out}↓ {cached}⎙ | {model_display}"`.
pub(crate) fn render_status_bar(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    let left = status_left(&state.status, state.output.selection.as_ref());
    let right = status_right(&state.status);
    let padded = pad_status_line(left.as_str(), right.as_str(), area.width as usize);
    frame.render_widget(Paragraph::new(padded), area);
}

/// Build the left side of the status bar from cwd, optional git branch, selection state,
/// and optional backoff countdown.
///
/// When `has_selection` is true, appends `" | c to copy"` after the branch to indicate
/// that the 'c' key will copy the selected text to the clipboard.
/// When a `backoff_until` deadline is set and in the future, appends `" | [Backoff: Xs]"`
/// showing the remaining wait in whole seconds.
pub fn status_left(status: &StatusBarData, selection: Option<&OutputSelection>) -> StatusLabel {
    let base = match &status.git_branch {
        Some(branch) => format!("{} [{}]", status.cwd, branch),
        None => status.cwd.to_string(),
    };
    let backoff_suffix = backoff_remaining_secs(status.context_window.backoff_until)
        .map(|s| format!(" | [Backoff: {}s]", s))
        .unwrap_or_default();
    let base_with_backoff = format!("{}{}", base, backoff_suffix);
    if selection.is_some() {
        StatusLabel::new(format!("{} | c to copy", base_with_backoff))
    } else {
        StatusLabel::new(base_with_backoff)
    }
}

/// Compute the remaining seconds until `backoff_until`, or `None` when not in backoff.
///
/// Returns `None` when `backoff_until` is `None` or when the deadline has already
/// passed (saturating to zero). Returns `Some(remaining_secs)` otherwise.
/// Consumers: `status_left` on every render frame.
fn backoff_remaining_secs(backoff_until: Option<std::time::Instant>) -> Option<u64> {
    let deadline = backoff_until?;
    let remaining = deadline.saturating_duration_since(std::time::Instant::now());
    if remaining.is_zero() {
        None
    } else {
        Some(remaining.as_secs())
    }
}

/// Format a token count as a compact display string.
///
/// Values strictly greater than 1,000 / 1,000,000 / 1,000,000,000 are rendered
/// as abbreviated `"Nk"` / `"Nm"` / `"Nb"` labels with one decimal place of
/// precision (rounded to nearest tenth). Trailing `.0` is omitted. All other
/// values are rendered as plain integers.
///
/// # Examples
///
/// ```text
/// format_token_count(42_149)        → "42.1k"
/// format_token_count(2_500_000)     → "2.5m"
/// format_token_count(3_600_000_000) → "3.6b"
/// format_token_count(1_000)         → "1000"
/// ```
const THOUSANDS_DIVISOR: u64 = 1_000;
const MILLIONS_DIVISOR: u64 = 1_000_000;
const BILLIONS_DIVISOR: u64 = 1_000_000_000;

fn format_abbreviated_count(n: u64, divisor: u64, suffix: &str) -> String {
    let value = n as f64 / divisor as f64;
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}{}", rounded as u64, suffix)
    } else {
        format!("{rounded:.1}{suffix}")
    }
}

/// Return a compact token-count label used in the footer status bar.
fn format_token_count(n: u64) -> String {
    if n > BILLIONS_DIVISOR {
        return format_abbreviated_count(n, BILLIONS_DIVISOR, "b");
    }
    if n > MILLIONS_DIVISOR {
        return format_abbreviated_count(n, MILLIONS_DIVISOR, "m");
    }
    if n > THOUSANDS_DIVISOR {
        return format_abbreviated_count(n, THOUSANDS_DIVISOR, "k");
    }
    format!("{}", n)
}

/// Format a USD cost value as `"$X.XX"` for display in the status bar.
///
/// Returns an empty string when `cost` is zero so callers can skip the segment
/// without an extra branch.
pub(crate) fn format_cost(cost: UsdCost) -> StatusLabel {
    if cost == 0.0 {
        return StatusLabel::new("");
    }
    StatusLabel::new(format!("${:.2}", *cost))
}

/// Format the context window usage as `"ctx N/Mk"` where N is current tokens
/// and M is the token limit.
///
/// Returns an empty string when `token_limit` is zero (limit unknown).
/// The token counts use the compact `"Nk"` notation from `\`format_token_count\``.
///
/// # Examples
///
/// ```text
/// // stats = ContextUsageStats { current_tokens: 5_000, token_limit: 200_000, messages_length: 4 }
/// format_context_window(&stats) → "ctx 5k/200k"
/// // no_limit = ContextUsageStats { token_limit: 0, ... }
/// format_context_window(&no_limit) → ""
/// ```
fn format_context_window(stats: &augur_domain::domain::types::ContextUsageStats) -> String {
    let limit = *stats.token_limit;
    let no_limit_known = limit == 0;
    if no_limit_known {
        return String::new();
    }
    let current_str = format_token_count(*stats.current_tokens);
    let limit_str = format_token_count(limit);
    format!("ctx {}/{}", current_str, limit_str)
}

/// Build the right side of the status bar.
///
/// Format: `"{in}↑ {out}↓ {cached}⎙"` followed by
/// `" | {model_display}"` when the model label is nonempty, and
/// `" | ctx N/Mk"` when `last_context` is `Some` and `token_limit > 0`.
///
/// The token counts use the compact `"Nk"` notation from `\`format_token_count\``.
///
/// Consumers: `render_status_bar` on every frame.
pub fn status_right(status: &StatusBarData) -> StatusLabel {
    let in_str = format_token_count(*status.token_totals.tokens_in);
    let out_str = format_token_count(*status.token_totals.tokens_out);
    let cached_str = format_token_count(*status.token_totals.tokens_cached);
    let mut s = format!("{}↑ {}↓ {}⎙", in_str, out_str, cached_str);
    let model = status.model_display.as_str();
    if !model.is_empty() {
        s.push_str(" | ");
        s.push_str(model);
    }
    // Append context window usage when last_context is Some and token_limit > 0.
    // When token_limit == 0 (unknown), format_context_window returns "" and is skipped.
    if let Some(ref ctx) = status.last_context {
        let ctx_str = format_context_window(ctx);
        if !ctx_str.is_empty() {
            s.push_str(" | ");
            s.push_str(&ctx_str);
        }
    }
    // Append cost segment when cost_usd > 0.0.
    let cost_str = format_cost(status.token_totals.cost_usd);
    if !cost_str.as_str().is_empty() {
        s.push_str(" | ");
        s.push_str(cost_str.as_str());
    }
    StatusLabel::new(s)
}

/// Pad `left` and `right` with spaces to fill exactly `width` display columns.
///
/// When the combined length exceeds `width`, a single space is inserted between
/// them to preserve readability. Character count (not bytes) is used for width
/// measurement to handle multi-byte Unicode in the token symbols correctly.
fn pad_status_line(left: &str, right: &str, width: usize) -> String {
    let left_chars = left.chars().count();
    let right_chars = right.chars().count();
    let total = left_chars + right_chars;
    let gap = if total < width { width - total } else { 1 };
    format!("{}{}{}", left, " ".repeat(gap), right)
}
