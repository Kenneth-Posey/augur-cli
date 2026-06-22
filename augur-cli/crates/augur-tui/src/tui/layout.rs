//! Terminal layout calculation. Pure functions; no terminal I/O.

use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::PromptText;
use ratatui::layout::Rect;

/// Width in characters of the standard prompt prefix (`"❯ "`).
const PROMPT_PREFIX_WIDTH: u16 = 2;
/// Whole-number denominator used for percentage-based width splits.
const PERCENT_DENOMINATOR: u32 = 100;
/// Minimum number of rows kept visible for wrapped text and output panes.
const MIN_VISIBLE_ROWS: u16 = 1;
/// Minimum number of rows reserved for the inline query UI (question + input).
const MIN_QUERY_INPUT_ROWS: u16 = 2;
/// Width of the gutter between the primary feed and secondary container.
const SECONDARY_LAYOUT_GUTTER_COLS: u16 = 1;
/// Columns that must remain outside the secondary pane (primary + gutter).
const SECONDARY_LAYOUT_RESERVED_NON_SECONDARY_COLS: u16 = 2;

/// Fixed overhead rows: 2 separators + 1 blank row above thinking + 1 thinking row + 1 status bar + 1 blank row at the bottom.
///
/// Updated from 5 to 6 to account for the blank spacing row added above the
/// thinking spinner between the output pane and the thinking row.
pub const LAYOUT_FIXED_ROWS: u16 = 6;

/// Maximum rows reserved for the command hint area above the input box.
///
/// Caps `hint_rows` in `LayoutSizes` so a long command list cannot crowd the
/// output pane. Passed as `hint_count` to `compute_layout` by `render_chat`.
pub const MAX_HINT_ROWS: u16 = 10;

/// Input parameters for computing the chat layout row allocation.
#[derive(bon::Builder)]
pub struct ChatLayoutInput<'a> {
    /// Total terminal rows available for the conversation screen.
    pub terminal_rows: u16,
    /// Total terminal columns available for the conversation screen.
    pub terminal_cols: u16,
    /// Current prompt buffer text used to determine wrapped input height.
    pub input_text: &'a str,
    /// Number of active completion hints shown above the input box.
    pub hint_count: Count,
}

/// Computed row allocation for output, hint, and input areas under the chat layout.
#[derive(bon::Builder)]
pub struct LayoutSizes {
    /// Rows available to the scrollable output pane (top zone).
    pub output_rows: u16,
    /// Rows reserved for the command hint area (0 when no hints are active).
    pub hint_rows: u16,
    /// Rows reserved for the dynamic input area (expands with text length).
    pub input_rows: u16,
}

/// Compute the display height of the input area from text length and terminal width.
///
/// Accounts for the `PROMPT_PREFIX_WIDTH`-char `"❯ "` prefix. Uses char count (not bytes) to handle
/// multi-byte Unicode correctly. Returns at least 1 even when `cols` is zero or
/// the text is empty. Called by both `compute_layout` and `render_chat`.
pub fn compute_input_height(text: &PromptText, cols: Count) -> Count {
    if cols.inner() == 0 {
        return Count::new(MIN_VISIBLE_ROWS as usize);
    }
    let col_count = cols.inner() as u16;
    let display_len = PROMPT_PREFIX_WIDTH + text.chars().count() as u16;
    let rows = display_len.div_ceil(col_count).max(MIN_VISIBLE_ROWS);
    Count::new(rows as usize)
}

/// Compute output, hint, and input row counts given terminal dimensions and current state.
///
/// Subtracts `LAYOUT_FIXED_ROWS`, `hint_rows` (capped at `MAX_HINT_ROWS`), and
/// `input_rows` from `terminal_rows`. `output_rows` is clamped to at least 1 so
/// the output pane is always visible. Used by `render_chat` to drive the chat
/// `Layout::vertical` split.
pub fn compute_layout(input: ChatLayoutInput<'_>) -> LayoutSizes {
    let input_rows = compute_input_height(
        &PromptText::from(input.input_text),
        Count::new(input.terminal_cols as usize),
    );
    let hint_rows = (input.hint_count.inner() as u16).min(MAX_HINT_ROWS);
    let overhead = LAYOUT_FIXED_ROWS
        .saturating_add(input_rows.inner() as u16)
        .saturating_add(hint_rows);
    let output_rows = input
        .terminal_rows
        .saturating_sub(overhead)
        .max(MIN_VISIBLE_ROWS);
    LayoutSizes::builder()
        .output_rows(output_rows)
        .hint_rows(hint_rows)
        .input_rows(input_rows.inner() as u16)
        .build()
}

/// Percentage of the terminal width allocated to the plan panel right zone.
///
/// At 25%, a 200-column terminal gives the panel 50 columns and the chat area
/// 150 columns. The panel is also subject to `MIN_PLAN_PANEL_COLS`.
/// Consumers: `compute_plan_layout`, `render_plan_layout`.
pub const PLAN_PANEL_WIDTH_PERCENT: u16 = 25;

/// Minimum column width enforced for the plan panel regardless of terminal size.
///
/// Prevents the panel from becoming too narrow to read on small terminals.
/// When the 25% calculation falls below this floor, this value is used instead
/// and the chat area shrinks to absorb the difference.
const MIN_PLAN_PANEL_COLS: u16 = 20;

/// Column widths for the horizontal plan-mode split.
#[derive(bon::Builder)]
pub struct PlanLayoutWidths {
    /// Columns allocated to the left chat zone.
    pub chat_cols: u16,
    /// Columns allocated to the right plan panel zone.
    pub panel_cols: u16,
}

/// Compute the horizontal column split for plan mode.
///
/// Allocates `PLAN_PANEL_WIDTH_PERCENT`% of `total_width` to the plan panel,
/// with a minimum of `MIN_PLAN_PANEL_COLS` (20). The chat zone receives the
/// remainder. The two widths always sum to `total_width`.
/// Called by `render_plan_layout` in `screens/conversation.rs`.
pub fn compute_plan_layout(total_width: Count) -> PlanLayoutWidths {
    let raw_panel =
        (total_width.inner() as u32 * PLAN_PANEL_WIDTH_PERCENT as u32 / PERCENT_DENOMINATOR) as u16;
    let total_width = total_width.inner() as u16;
    let panel_cols = raw_panel.max(MIN_PLAN_PANEL_COLS).min(total_width);
    let chat_cols = total_width.saturating_sub(panel_cols);
    PlanLayoutWidths::builder()
        .chat_cols(chat_cols)
        .panel_cols(panel_cols)
        .build()
}

/// Column widths for the three-pane layout when both the secondary container and
/// the plan panel are active simultaneously.
#[derive(bon::Builder)]
pub struct ThreePaneLayout {
    /// Columns for the conversation area (primary feed + optional secondary container combined).
    pub conversation_cols: u16,
    /// Columns for the plan/guided-plan panel.
    pub plan_panel_cols: u16,
}

/// Compute three-pane layout when both secondary container and plan panel are active.
///
/// Applies the plan panel percentage (25%) first using [`compute_plan_layout`], then
/// the remaining width is the conversation area. The secondary split within the
/// conversation area is handled separately by [`compute_secondary_layout`] applied to
/// the conversation rect. The two widths always sum to `total_width`.
///
/// Called by `screens::conversation::render_plan_layout` and
/// `screens::conversation::render_guided_plan_layout` when
/// `state.interaction.panel.secondary_view.is_some()`.
pub fn compute_three_pane_layout(total_width: Count) -> ThreePaneLayout {
    let plan = compute_plan_layout(total_width);
    ThreePaneLayout::builder()
        .conversation_cols(plan.chat_cols)
        .plan_panel_cols(plan.panel_cols)
        .build()
}

/// Input parameters for computing inline query input height.
#[derive(bon::Builder)]
pub struct QueryInputRowsInput<'a> {
    /// Query question text shown above the choices/free-form input.
    pub question: &'a str,
    /// Number of available query choices.
    pub choice_count: Count,
    /// Current free-form response text.
    pub freeform: &'a str,
    /// Available columns for wrapping the query content.
    pub cols: u16,
}

/// Compute the number of display rows the question text occupies given terminal width.
///
/// Splits the question by `\n` and computes wrapped row count per segment using
/// char count divided by `cols`. Returns at least 1 even for an empty question.
/// Used by `compute_query_input_rows` to size the question section of the layout.
fn question_display_rows(question: &str, cols: u16) -> Count {
    if cols == 0 {
        return Count::new((question.lines().count() as u16).max(MIN_VISIBLE_ROWS) as usize);
    }
    let total: u16 = question
        .lines()
        .map(|seg| {
            let len = seg.chars().count() as u16;
            if len == 0 {
                MIN_VISIBLE_ROWS
            } else {
                len.div_ceil(cols)
            }
        })
        .sum();
    Count::new(total.max(MIN_VISIBLE_ROWS) as usize)
}

/// Compute the number of input rows needed for the inline query input area.
///
/// Accounts for multi-line question wrapping based on terminal width. Returns the
/// sum of question rows (wrapping-aware), one row per choice, and the number of
/// rows needed for the free-form input line (which wraps like the prompt input).
/// Minimum of 2 (question + freeform) even when the choice list is empty so both
/// areas are always visible. Called by `render_query_inline` to compute the query
/// input zone height before `split_layout`.
pub fn compute_query_input_rows(input: QueryInputRowsInput<'_>) -> Count {
    let question_rows = question_display_rows(input.question, input.cols);
    let freeform_rows = if input.cols == 0 {
        Count::new(MIN_VISIBLE_ROWS as usize)
    } else {
        let freeform_chars = input.freeform.chars().count() as u16;
        Count::new(
            (PROMPT_PREFIX_WIDTH + freeform_chars)
                .div_ceil(input.cols)
                .max(MIN_VISIBLE_ROWS) as usize,
        )
    };
    Count::new(
        question_rows
            .inner()
            .saturating_add(input.choice_count.inner())
            .saturating_add(freeform_rows.inner())
            .max(MIN_QUERY_INPUT_ROWS as usize),
    )
}

/// Percentage of the terminal width allocated to the primary feed pane when the
/// secondary container is open.
///
/// At 65%, a 100-column terminal gives the primary pane 65 columns, the gutter 1 column,
/// and the secondary container the remaining 34 columns (~34% of screen width).
/// Consumers: `compute_secondary_layout`, `render_conversation_container`.
pub(crate) const PRIMARY_FEED_WIDTH_PERCENT: u16 = 65;

/// Column split for the horizontal secondary-container layout.
///
/// Produced by `compute_secondary_layout` and consumed by
/// `components::conversation_container::render_conversation_container`.
/// The three rects span the full container height and together fill the full width.
#[derive(bon::Builder)]
pub struct SecondaryLayout {
    /// Left pane rect for the primary feed output (65% of container width).
    pub primary_rect: Rect,
    /// 1-column gutter between the primary and secondary panes.
    pub gutter_rect: Rect,
    /// Right pane rect for the secondary container (remaining width).
    pub secondary_rect: Rect,
}

/// Compute the horizontal column split for the secondary-container layout.
///
/// Allocates the secondary pane as `(100 - PRIMARY_FEED_WIDTH_PERCENT)`% of
/// `reference_width`, clamped so it fits within `area.width` (leaving room for
/// at least the 1-column gutter). The primary pane receives whatever width remains
/// in `area` after the secondary and gutter are allocated. All three rects share
/// the `y` and `height` of `area`. The widths always sum to `area.width`. Both
/// primary and secondary are clamped to a minimum of 1 column.
///
/// # Parameters
///
/// - `area` - the spatial rect to fill (determines `x`, `y`, `height`, and the
///   ceiling on secondary/primary widths).
/// - `reference_width` - the terminal width used for the percentage calculation.
///   Pass the full terminal width when `area` is a sub-rect (e.g. the conversation
///   zone after carving off a plan panel), so the secondary pane is sized as a
///   fraction of the whole screen rather than a fraction of the already-reduced
///   conversation zone.  Pass `area.width` for the normal single-column case
///   (equivalent to [`compute_secondary_layout`]).
///
/// Called by `render_conversation_container` in `components::conversation_container`.
pub(crate) fn compute_secondary_layout_with_ref(
    area: Rect,
    reference_width: Count,
) -> SecondaryLayout {
    let secondary_cols = (((reference_width.inner() as u16) as u32
        * (PERCENT_DENOMINATOR - PRIMARY_FEED_WIDTH_PERCENT as u32))
        / PERCENT_DENOMINATOR) as u16;
    // Clamp secondary so it fits within area leaving room for the gutter - no min(1)
    // so very narrow terminals don't push the sum over area.width.
    let secondary_cols = secondary_cols.min(
        area.width
            .saturating_sub(SECONDARY_LAYOUT_RESERVED_NON_SECONDARY_COLS),
    );
    let gutter_cols = SECONDARY_LAYOUT_GUTTER_COLS;
    let primary_cols = area
        .width
        .saturating_sub(secondary_cols)
        .saturating_sub(gutter_cols);
    SecondaryLayout::builder()
        .primary_rect(Rect {
            x: area.x,
            y: area.y,
            width: primary_cols,
            height: area.height,
        })
        .gutter_rect(Rect {
            x: area.x + primary_cols,
            y: area.y,
            width: gutter_cols,
            height: area.height,
        })
        .secondary_rect(Rect {
            x: area.x + primary_cols + gutter_cols,
            y: area.y,
            width: secondary_cols,
            height: area.height,
        })
        .build()
}

/// Compute the horizontal column split for the secondary-container layout.
///
/// Delegates to `compute_secondary_layout_with_ref` with `reference_width =
/// area.width`, so the secondary pane is sized as a percentage of the same
/// `area` that is being split. Use `compute_secondary_layout_with_ref` when
/// `area` is a sub-rect of the full terminal and you want the secondary pane
/// sized relative to the full terminal width instead.
///
/// Called by `render_conversation_container` in `components::conversation_container`.
pub fn compute_secondary_layout(area: Rect) -> SecondaryLayout {
    compute_secondary_layout_with_ref(area, Count::new(area.width as usize))
}

/// Split `area` into the main content rect (all but the last row) and the controls row rect.
///
/// Returns `(main_area, controls_area)`. When `area.height` is 0 or 1, the
/// main area is returned unchanged and the controls rect has zero height.
/// Called by `screens::conversation::render_conversation` to carve off the
/// bottom controls row before computing the vertical chat layout.
pub(crate) fn split_controls_area(area: Rect) -> (Rect, Rect) {
    if area.height <= 1 {
        return (area, Rect::default());
    }
    let main = Rect {
        height: area.height - 1,
        ..area
    };
    let controls = Rect {
        y: area.y + area.height - 1,
        height: 1,
        ..area
    };
    (main, controls)
}

/// Layout descriptor for the conversation render area.
///
/// `area` is the rect allocated to the conversation column.
/// `reference_width` is the full terminal width used for secondary-pane
/// percentage calculations. When `None`, `area.width` is used as the reference
/// (chat mode). When `Some(w)`, `w` is used instead (plan mode, where `area`
/// is narrower than the terminal).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConversationArea {
    pub(crate) area: Rect,
    pub(crate) reference_width: Option<u16>,
}

impl ConversationArea {
    /// Construct a `ConversationArea` for chat/query mode, where `area` already
    /// spans the full terminal width.
    pub(crate) fn full(area: Rect) -> Self {
        Self {
            area,
            reference_width: None,
        }
    }

    /// Construct a `ConversationArea` for plan mode, where `area` is narrower
    /// than the terminal and `terminal_width` is the full terminal width.
    pub(crate) fn plan(area: Rect, terminal_width: Count) -> Self {
        Self {
            area,
            reference_width: Some(terminal_width.inner() as u16),
        }
    }
}
