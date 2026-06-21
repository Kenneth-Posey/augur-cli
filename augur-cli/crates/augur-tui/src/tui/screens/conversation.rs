//! Conversation screen: assembles the full conversation layout and dispatches
//! to mode-specific sub-layouts (chat, query, plan, guided plan).

mod guided_plan_panel;
mod layout_zones;
mod plan_layout;
mod query_input;

use crate::domain::tui_display_state::{
    DisplayConversationMode, QueryDisplayState, TuiDisplayState,
};
use crate::tui::components::conversation_container::{
    render_conversation_container, render_primary_feed_only,
};
use crate::tui::components::footer::{render_controls_row, render_status_bar};
use crate::tui::components::primary_feed::render_separator;
use crate::tui::components::text_entry::{active_hint_count, render_active_hints, render_input};
use crate::tui::layout::{
    compute_layout, compute_query_input_rows, split_controls_area, ChatLayoutInput,
    ConversationArea, QueryInputRowsInput,
};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use ratatui::layout::Rect;
use ratatui::Frame;

use layout_zones::{conv_area_above, split_layout};
use plan_layout::{
    render_guided_plan_layout, render_plan_layout, GuidedPlanLayoutContext, PlanLayoutContext,
};
use query_input::render_query_input;
pub use query_input::{build_inline_choice_lines, split_question_lines};

#[derive(bon::Builder)]
struct QueryLayoutContext<'a> {
    state: &'a TuiDisplayState,
    query_state: &'a QueryDisplayState,
    area: Rect,
}

/// Render the full conversation screen into `area`.
///
/// Carves off the bottom row as a controls row, then dispatches to the
/// mode-specific layout:
/// - `Chat` → `render_chat_layout`
/// - `Query(qs)` → `render_query_layout`
/// - `Plan(ps)` → `render_plan_layout`
/// - `GuidedPlan(gs)` → `render_guided_plan_layout`
///
/// Called by the shell dispatcher when `AppScreen::Conversation` is active.
pub(crate) fn render_conversation(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    let (main_area, controls_area) = split_controls_area(area);
    render_controls_row(frame, state, controls_area);
    match &state.interaction.mode {
        DisplayConversationMode::Chat => {
            render_chat_layout(frame, state, ConversationArea::full(main_area))
        }
        DisplayConversationMode::Query(qs) => render_query_layout(
            frame,
            QueryLayoutContext::builder()
                .state(state)
                .query_state(qs)
                .area(main_area)
                .build(),
        ),
        DisplayConversationMode::Plan(ps) => {
            render_plan_layout(frame, PlanLayoutContext::new(state, ps, main_area))
        }
        DisplayConversationMode::GuidedPlan(gs) => {
            render_guided_plan_layout(frame, GuidedPlanLayoutContext::new(state, gs, main_area))
        }
    }
}

/// Render the standard chat layout (with or without secondary container).
///
/// Computes vertical zones then delegates the conversation area to
/// `render_conversation_container` which handles the secondary split internally.
///
/// # Parameters
///
/// - `conv_area` - layout descriptor forwarded to [`render_conversation_container`].
///   Use [`ConversationArea::full`] when the area already spans the full terminal
///   (chat/query modes), or [`ConversationArea::plan`] with the full terminal width
///   when the area is a sub-rect (plan mode).
fn render_chat_layout(frame: &mut Frame, state: &TuiDisplayState, conv_area: ConversationArea) {
    let area = conv_area.area;
    let hint_count = active_hint_count(state);
    let layout = compute_layout(
        ChatLayoutInput::builder()
            .terminal_rows(area.height)
            .terminal_cols(area.width)
            .input_text(&state.prompt.buffer)
            .hint_count(hint_count)
            .build(),
    );
    let zones = split_layout(
        area,
        Count::new(layout.input_rows as usize),
        Count::new(layout.hint_rows as usize),
    );
    let chat_area = conv_area_above(area, zones.top_sep_above_input);

    render_conversation_container(
        frame,
        state,
        ConversationArea {
            area: chat_area,
            ..conv_area
        },
    );
    render_separator(frame, zones.top_sep_above_input);
    render_active_hints(frame, state, zones.bottom.hints);
    render_input(frame, state, zones.bottom.input);
    render_separator(frame, zones.bottom.sep_below_input);
    render_status_bar(frame, state, zones.bottom.status);
}

/// Render the chat layout with primary feed only - secondary container suppressed.
///
/// Identical to `render_chat_layout` except it calls `render_primary_feed_only`
/// instead of `render_conversation_container`, so the secondary container is not
/// rendered regardless of `state.interaction.panel.secondary_view`.
///
/// Used as a fallback by `render_plan_layout` and `render_guided_plan_layout` when
/// the three-pane layout would make the secondary pane narrower than 10 columns.
fn render_chat_layout_primary_only(frame: &mut Frame, state: &TuiDisplayState, area: Rect) {
    let hint_count = active_hint_count(state);
    let layout = compute_layout(
        ChatLayoutInput::builder()
            .terminal_rows(area.height)
            .terminal_cols(area.width)
            .input_text(&state.prompt.buffer)
            .hint_count(hint_count)
            .build(),
    );
    let zones = split_layout(
        area,
        Count::new(layout.input_rows as usize),
        Count::new(layout.hint_rows as usize),
    );
    let conv_area = conv_area_above(area, zones.top_sep_above_input);

    render_primary_feed_only(frame, state, conv_area);
    render_separator(frame, zones.top_sep_above_input);
    render_active_hints(frame, state, zones.bottom.hints);
    render_input(frame, state, zones.bottom.input);
    render_separator(frame, zones.bottom.sep_below_input);
    render_status_bar(frame, state, zones.bottom.status);
}

/// Render the query overlay: question + choices + freeform above the chat output.
///
/// Replaces the input zone with the query UI; no command hint rows are allocated.
fn render_query_layout(frame: &mut Frame, context: QueryLayoutContext<'_>) {
    let input_rows = compute_query_input_rows(
        QueryInputRowsInput::builder()
            .question(&context.query_state.question)
            .choice_count(augur_domain::domain::newtypes::Count::of(
                context.query_state.choices.len(),
            ))
            .freeform(&context.query_state.freeform)
            .cols(context.area.width)
            .build(),
    );
    let zones = split_layout(context.area, input_rows, Count::new(0));
    let conv_area = conv_area_above(context.area, zones.top_sep_above_input);

    render_conversation_container(frame, context.state, ConversationArea::full(conv_area));
    render_separator(frame, zones.top_sep_above_input);
    render_query_input(frame, context.query_state, zones.bottom.input);
    render_separator(frame, zones.bottom.sep_below_input);
    render_status_bar(frame, context.state, zones.bottom.status);
}
