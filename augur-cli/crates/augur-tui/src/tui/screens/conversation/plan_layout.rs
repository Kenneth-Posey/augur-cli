use crate::domain::tui_display_state::TuiDisplayState;
use crate::domain::tui_state::{GuidedPlanUiState, PlanModeState};
use crate::tui::layout::{
    compute_plan_layout, compute_three_pane_layout, ConversationArea, PRIMARY_FEED_WIDTH_PERCENT,
};
use crate::tui::plan_panel::{render_plan_panel, PlanPanelRender};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::Frame;

use super::guided_plan_panel::render_guided_plan_panel;

const MIN_SECONDARY_PANE_COLS: u16 = 10;
const PERCENT_BASIS: u32 = 100;

pub(super) struct PlanLayoutContext<'a> {
    state: &'a TuiDisplayState,
    plan_state: &'a PlanModeState,
    area: Rect,
}

impl<'a> PlanLayoutContext<'a> {
    /// Build the plan-panel render context from the parent screen inputs.
    pub(super) fn new(
        state: &'a TuiDisplayState,
        plan_state: &'a PlanModeState,
        area: Rect,
    ) -> Self {
        Self {
            state,
            plan_state,
            area,
        }
    }
}

pub(super) struct GuidedPlanLayoutContext<'a> {
    state: &'a TuiDisplayState,
    guided_plan_state: &'a GuidedPlanUiState,
    area: Rect,
}

impl<'a> GuidedPlanLayoutContext<'a> {
    /// Build the guided-plan render context from the parent screen inputs.
    pub(super) fn new(
        state: &'a TuiDisplayState,
        guided_plan_state: &'a GuidedPlanUiState,
        area: Rect,
    ) -> Self {
        Self {
            state,
            guided_plan_state,
            area,
        }
    }
}

/// Render plan mode using the extracted plan-layout helpers.
pub(super) fn render_plan_layout(frame: &mut Frame, context: PlanLayoutContext<'_>) {
    let panel_context = PanelLayoutContext::new(context.state, context.area, "plan");
    render_plan_mode(frame, panel_context, |frame, area| {
        render_plan_panel(
            frame,
            PlanPanelRender::builder()
                .tree(&context.plan_state.tree)
                .scroll(context.plan_state.tree_scroll)
                .area(area)
                .build(),
        );
    });
}

/// Render guided-plan mode using the extracted plan-layout helpers.
pub(super) fn render_guided_plan_layout(frame: &mut Frame, context: GuidedPlanLayoutContext<'_>) {
    let panel_context = PanelLayoutContext::new(context.state, context.area, "guided-plan");
    render_plan_mode(frame, panel_context, |frame, area| {
        render_guided_plan_panel(frame, context.guided_plan_state, area);
    });
}

#[derive(Clone, Copy)]
struct PanelLayoutContext<'a> {
    state: &'a TuiDisplayState,
    area: Rect,
    layout_name: &'static str,
}

impl<'a> PanelLayoutContext<'a> {
    fn new(state: &'a TuiDisplayState, area: Rect, layout_name: &'static str) -> Self {
        Self {
            state,
            area,
            layout_name,
        }
    }
}

fn render_plan_mode(
    frame: &mut Frame,
    panel_context: PanelLayoutContext<'_>,
    render_panel: impl Fn(&mut Frame, Rect),
) {
    if panel_context
        .state
        .interaction
        .panel
        .secondary_view
        .is_some()
    {
        render_three_pane_layout(frame, panel_context, render_panel);
    } else {
        render_split_plan_layout(frame, panel_context, render_panel);
    }
}

fn render_three_pane_layout(
    frame: &mut Frame,
    context: PanelLayoutContext<'_>,
    render_panel: impl Fn(&mut Frame, Rect),
) {
    let three = compute_three_pane_layout(Count::new(context.area.width as usize));
    let conversation_rect = Rect {
        width: three.conversation_cols,
        ..context.area
    };
    let panel_rect = Rect {
        x: context.area.x + three.conversation_cols,
        width: three.plan_panel_cols,
        ..context.area
    };

    context
        .state
        .output
        .panel_areas
        .plan_panel_area
        .set(panel_rect);
    render_plan_conversation(frame, context, conversation_rect);
    render_panel(frame, panel_rect);
}

fn render_split_plan_layout(
    frame: &mut Frame,
    context: PanelLayoutContext<'_>,
    render_panel: impl Fn(&mut Frame, Rect),
) {
    let widths = compute_plan_layout(Count::new(context.area.width as usize));
    let panes = Layout::horizontal([
        Constraint::Length(widths.chat_cols),
        Constraint::Length(widths.panel_cols),
    ])
    .split(context.area);

    super::render_chat_layout(frame, context.state, ConversationArea::full(panes[0]));
    render_panel(frame, panes[1]);
    context
        .state
        .output
        .panel_areas
        .plan_panel_area
        .set(panes[1]);
}

fn render_plan_conversation(
    frame: &mut Frame,
    context: PanelLayoutContext<'_>,
    conversation_rect: Rect,
) {
    if has_effective_secondary_pane(conversation_rect.width) {
        super::render_chat_layout(
            frame,
            context.state,
            ConversationArea::plan(conversation_rect, Count::new(context.area.width as usize)),
        );
    } else {
        log_collapsed_secondary(
            context.layout_name,
            estimated_secondary_cols(conversation_rect.width),
        );
        super::render_chat_layout_primary_only(frame, context.state, conversation_rect);
    }
}

fn has_effective_secondary_pane(conversation_cols: u16) -> bool {
    estimated_secondary_cols(conversation_cols) >= MIN_SECONDARY_PANE_COLS
}

fn estimated_secondary_cols(conversation_cols: u16) -> u16 {
    (conversation_cols as u32 * (PERCENT_BASIS - PRIMARY_FEED_WIDTH_PERCENT as u32) / PERCENT_BASIS)
        as u16
}

fn log_collapsed_secondary(layout_name: &str, secondary_width_estimate: u16) {
    tracing::debug!(
        "{layout_name} three-pane: secondary collapsed (estimated {secondary_width_estimate} cols < {MIN_SECONDARY_PANE_COLS} minimum)"
    );
}
