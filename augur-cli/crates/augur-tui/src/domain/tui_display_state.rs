//! Display-only projection of [`crate::domain::tui_state::AppState`] that is safe to `Clone` and send
//! across the actor → render-loop boundary via a `watch` channel.
//!
//! [`crate::domain::tui_state::AppState`] cannot be `Clone` because [`crate::domain::tui_state::ConversationMode::Query`] owns a
//! `oneshot::Sender`. This module provides [`TuiDisplayState`], a parallel
//! projection that replaces the non-`Clone` parts with equivalent display-only
//! types:
//!
//! - [`QueryDisplayState`] - [`QueryState`] minus `reply_tx`.
//! - [`DisplayConversationMode`] - mirrors [`crate::domain::tui_state::ConversationMode`] using
//!   [`QueryDisplayState`] for the `Query` variant.
//! - [`DisplayAppInteraction`] - mirrors [`crate::domain::tui_state::AppInteraction`] using
//!   [`DisplayConversationMode`] for `mode`.
//! - [`TuiDisplayState`] - the full projection of [`crate::domain::tui_state::AppState`].
//!
//! The render loop never writes back into `TuiDisplayState`; feedback from the
//! render path travels via [`RenderFeedback`].

use crate::domain::tui_state::{
    AgentStatus, AppScreen, GuidedPlanUiState, OutputPane, PanelOverlayState, PlanModeState,
    PromptPane, QueryState, StatusBarData,
};
use augur_domain::domain::IsPredicate;
use augur_domain::domain::newtypes::ScrollOffset;
use augur_domain::domain::string_newtypes::{ChoiceText, EndpointName, PromptText};

/// Display-only projection of [`QueryState`]: identical to [`QueryState`] but
/// without the `reply_tx` oneshot sender, making it safe to `Clone`.
///
/// Produced by [`TuiDisplayState::project_from`] when the active
/// [`crate::domain::tui_state::ConversationMode`] is `Query`. The render path uses this to draw the
/// query overlay; the actor retains the real [`QueryState`] (with `reply_tx`)
/// for command dispatch.
///
/// Parameters:
/// - `question`: question text displayed at the top of the overlay.
/// - `choices`: optional list of choices the user can navigate.
/// - `selected`: index of the highlighted choice, or `None`.
/// - `freeform`: free-form text typed by the user.
#[derive(Clone, bon::Builder)]
pub struct QueryDisplayState {
    /// The question text displayed at the top of the overlay.
    pub question: PromptText,
    /// Optional choices the user can navigate with up/down arrows.
    pub choices: Vec<ChoiceText>,
    /// Index of the currently highlighted choice, or `None`.
    pub selected: Option<usize>,
    /// Free-form text the user has typed; takes priority over a selected choice.
    pub freeform: PromptText,
}

impl QueryDisplayState {
    /// Project a [`QueryState`] into a [`QueryDisplayState`], discarding `reply_tx`.
    ///
    /// Parameters:
    /// - `state`: the full query state held by the TUI actor.
    ///
    /// Returns: a display-only clone of the query fields.
    pub fn project_from(state: &QueryState) -> Self {
        QueryDisplayState::builder()
            .question(state.question.clone())
            .choices(state.choices.clone())
            .maybe_selected(state.selected)
            .freeform(state.freeform.clone())
            .build()
    }
}

/// Display-only mirror of [`crate::domain::tui_state::ConversationMode`].
///
/// Identical structure but uses [`QueryDisplayState`] for the `Query` variant
/// so the entire enum is `Clone`. Produced by [`DisplayAppInteraction::project_from`].
#[derive(Clone)]
pub enum DisplayConversationMode {
    /// Normal chat interaction mode.
    Chat,
    /// Query overlay mode; display-only projection of the query state.
    Query(QueryDisplayState),
    /// Plan mode: chat on the left 75%, plan tree panel on the right 25%.
    Plan(PlanModeState),
    /// Guided plan execution mode: chat + phase panel.
    GuidedPlan(GuidedPlanUiState),
}

impl DisplayConversationMode {
    /// Project a [`crate::domain::tui_state::ConversationMode`] into a
    /// [`DisplayConversationMode`], discarding any non-`Clone` fields.
    ///
    /// Parameters:
    /// - `mode`: the active conversation mode held by the TUI actor.
    ///
    /// Returns: a display-only clone of the mode.
    pub fn project_from(mode: &crate::domain::tui_state::ConversationMode) -> Self {
        use crate::domain::tui_state::ConversationMode;
        match mode {
            ConversationMode::Chat => DisplayConversationMode::Chat,
            ConversationMode::Query(q) => {
                DisplayConversationMode::Query(QueryDisplayState::project_from(q))
            }
            ConversationMode::Plan(p) => DisplayConversationMode::Plan(p.clone()),
            ConversationMode::GuidedPlan(g) => DisplayConversationMode::GuidedPlan(g.clone()),
        }
    }
}

/// Display-only mirror of [`crate::domain::tui_state::AppInteraction`].
///
/// Uses [`DisplayConversationMode`] for `mode`, making the struct `Clone`.
/// Produced by [`TuiDisplayState::project_from`].
///
/// Parameters:
/// - `screen`: current full-screen context.
/// - `mode`: active conversation mode (display projection).
/// - `panel`: secondary-panel overlay state.
#[derive(Clone, bon::Builder)]
pub struct DisplayAppInteraction {
    /// Current full-screen context: session selector or conversation.
    pub screen: AppScreen,
    /// Active conversation mode (display-only projection).
    pub mode: DisplayConversationMode,
    /// Secondary-panel overlay state and focus.
    pub panel: PanelOverlayState,
}

impl DisplayAppInteraction {
    /// Project an [`crate::domain::tui_state::AppInteraction`] into a
    /// [`DisplayAppInteraction`].
    ///
    /// Parameters:
    /// - `interaction`: the full interaction state held by the TUI actor.
    ///
    /// Returns: a display-only clone of the interaction fields.
    pub fn project_from(interaction: &crate::domain::tui_state::AppInteraction) -> Self {
        DisplayAppInteraction::builder()
            .screen(interaction.screen.clone())
            .mode(DisplayConversationMode::project_from(&interaction.mode))
            .panel(interaction.panel.clone())
            .build()
    }
}

/// Feedback sent from the render loop back to the TUI actor after each frame.
///
/// The render path mutates interior-mutable fields on the [`TuiDisplayState`]
/// clone and then packages those mutations here so the actor can apply them to
/// its authoritative [`crate::domain::tui_state::AppState`].
///
/// Parameters:
/// - `panel_areas`: updated panel bounding rectangles computed during this frame.
/// - `scroll_offset`: recalculated scroll offset (may change on terminal resize).
#[derive(Clone, Default, bon::Builder)]
pub struct RenderFeedback {
    /// Panel bounding rectangles as recorded by the render path for this frame.
    pub panel_areas: crate::domain::tui_state::PanelAreas,
    /// Recalculated scroll offset for the primary output pane.
    pub scroll_offset: ScrollOffset,
}

/// Clone-able projection of [`crate::domain::tui_state::AppState`] used as the
/// unit of transfer across the actor → render-loop `watch` channel.
///
/// Mirrors the five fields of `AppState` but replaces [`crate::domain::tui_state::AppInteraction`]
/// with [`DisplayAppInteraction`] so the whole struct is `Clone`.
///
/// Invariant: `TuiDisplayState` is always derived from a live [`crate::domain::tui_state::AppState`] via
/// [`TuiDisplayState::project_from`]. It is never mutated after construction.
///
/// Parameters:
/// - `output`: output pane state.
/// - `prompt`: prompt pane state.
/// - `agent`: agent execution status.
/// - `status`: status bar data.
/// - `interaction`: display-only interaction state.
#[derive(Clone, bon::Builder)]
pub struct TuiDisplayState {
    /// Output pane state: accumulated lines and scroll position.
    pub output: OutputPane,
    /// Prompt pane state: input buffer and cursor.
    pub prompt: PromptPane,
    /// Agent execution status: endpoint name and thinking indicator.
    pub agent: AgentStatus,
    /// Status bar display data: tokens, model label, cwd, git branch.
    pub status: StatusBarData,
    /// Current display mode, ask panel overlay, and input focus state (display-only).
    pub interaction: DisplayAppInteraction,
}

impl TuiDisplayState {
    /// Construct the initial [`TuiDisplayState`] matching [`crate::domain::tui_state::AppState::new`].
    ///
    /// Parameters:
    /// - `endpoint`: the active endpoint name shown in the status bar.
    /// - `screen`: the initial full-screen context.
    ///
    /// Returns: a default-initialized [`TuiDisplayState`] ready to send on the
    /// watch channel before the first real frame.
    pub fn new(endpoint: EndpointName, screen: AppScreen) -> Self {
        use crate::domain::tui_state::{
            AgentFeedState, AgentStatus, OutputPane, PanelAreas, PanelOverlayState, PromptPane,
            StatusBarData, ThinkingIndicator,
        };

        TuiDisplayState::builder()
            .output(
                OutputPane::builder()
                    .lines(Vec::new())
                    .panel_areas(PanelAreas::default())
                    .build(),
            )
            .prompt(
                PromptPane::builder()
                    .buffer(String::new().into())
                    .cursor(0)
                    .completions(Default::default())
                    .models(Default::default())
                    .build(),
            )
            .agent(
                AgentStatus::builder()
                    .endpoint_name(endpoint)
                    .thinking(ThinkingIndicator::default())
                    .build(),
            )
            .status(StatusBarData::default())
            .interaction(
                DisplayAppInteraction::builder()
                    .screen(screen)
                    .mode(DisplayConversationMode::Chat)
                    .panel(
                        PanelOverlayState::builder()
                            .agent_feed(AgentFeedState::default())
                            .input_focus(Default::default())
                            .build(),
                    )
                    .build(),
            )
            .build()
    }

    /// Project a live [`crate::domain::tui_state::AppState`] into a
    /// [`TuiDisplayState`] by cloning all `Clone` fields and projecting
    /// the non-`Clone` [`crate::domain::tui_state::AppInteraction`].
    ///
    /// Parameters:
    /// - `state`: the authoritative app state owned by the TUI actor.
    ///
    /// Returns: a display-only snapshot suitable for sending on the watch channel.
    ///
    /// Side effects: none; `state` is not mutated.
    pub fn project_from(state: &crate::domain::tui_state::AppState) -> Self {
        TuiDisplayState::builder()
            .output(state.output.clone())
            .prompt(state.prompt.clone())
            .agent(state.agent.clone())
            .status(state.status.clone())
            .interaction(DisplayAppInteraction::project_from(&state.interaction))
            .build()
    }

    /// Return `true` when any tracked agent feed still has an active task.
    pub(crate) fn any_agent_feed_active(&self) -> IsPredicate {
        if self.interaction.panel.agent_feed.active_task.is_some() {
            return IsPredicate::yes();
        }
        IsPredicate::from(
            self.interaction
                .panel
                .agent_feed
                .feeds
                .iter()
                .any(|feed| feed.active_task.is_some()),
        )
    }
}
