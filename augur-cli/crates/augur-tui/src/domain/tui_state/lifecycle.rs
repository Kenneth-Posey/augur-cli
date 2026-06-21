//! Lifecycle and navigation helpers for `AppState`.

use super::*;
use crate::domain::tui_render::{line_display_rows, rendered_line_text};
use augur_domain::domain::newtypes::{Count, IsPredicate, ScrollOffset};
use augur_domain::domain::string_newtypes::StringNewtype;

impl AppState {
    /// Create an initial `AppState` with empty output and prompt.
    pub fn new(default_endpoint: EndpointName, screen: AppScreen) -> Self {
        AppState::builder()
            .output(
                OutputPane::builder()
                    .lines(vec![])
                    .panel_areas(PanelAreas::default())
                    .build(),
            )
            .prompt(
                PromptPane::builder()
                    .buffer(String::new().into())
                    .cursor(0)
                    .completions(PromptCompletions::default())
                    .models(ModelPickerData::default())
                    .build(),
            )
            .agent(
                AgentStatus::builder()
                    .endpoint_name(default_endpoint)
                    .thinking(ThinkingIndicator::default())
                    .build(),
            )
            .status(StatusBarData::default())
            .interaction(
                AppInteraction::builder()
                    .screen(screen)
                    .mode(ConversationMode::Chat)
                    .panel(
                        PanelOverlayState::builder()
                            .agent_feed(AgentFeedState::default())
                            .input_focus(InputFocus::Main)
                            .build(),
                    )
                    .build(),
            )
            .build()
    }

    /// Set the `guided_awaiting_compact` flag when entering guided-plan compact wait.
    pub fn set_guided_plan_compact_flag(&mut self) {
        if let ConversationMode::GuidedPlan(ref mut ui) = self.interaction.mode {
            ui.guided_awaiting_compact = true.into();
        }
    }

    /// Clear the `guided_awaiting_compact` flag after compaction completes.
    pub fn clear_guided_plan_compact_flag(&mut self) {
        if let ConversationMode::GuidedPlan(ref mut ui) = self.interaction.mode {
            ui.guided_awaiting_compact = false.into();
        }
    }

    /// Return `true` when any tracked agent feed is still active.
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

    /// Select the next tracked agent feed when one exists.
    pub(crate) fn select_next_agent_feed(&mut self) -> IsPredicate {
        let len = self.interaction.panel.agent_feed.feeds.len();
        if len < 2 {
            return IsPredicate::no();
        }
        let selected = self.interaction.panel.agent_feed.selected_feed.unwrap_or(0);
        let next = (selected + 1).min(len - 1);
        if next == selected {
            return IsPredicate::no();
        }
        self.interaction.panel.agent_feed.selected_feed = Some(next);
        self.sync_selected_agent_feed();
        IsPredicate::yes()
    }

    /// Select the previous tracked agent feed when one exists.
    pub(crate) fn select_prev_agent_feed(&mut self) -> IsPredicate {
        let len = self.interaction.panel.agent_feed.feeds.len();
        if len < 2 {
            return IsPredicate::no();
        }
        let selected = self.interaction.panel.agent_feed.selected_feed.unwrap_or(0);
        let prev = selected.saturating_sub(1);
        if prev == selected {
            return IsPredicate::no();
        }
        self.interaction.panel.agent_feed.selected_feed = Some(prev);
        self.sync_selected_agent_feed();
        IsPredicate::yes()
    }

    fn selected_agent_feed_index(&self) -> Option<usize> {
        self.interaction.panel.agent_feed.selected_feed
    }

    /// Sync panel-level feed mirrors from the currently selected agent feed row.
    pub(crate) fn sync_selected_agent_feed(&mut self) {
        let Some(selected_index) = self.selected_agent_feed_index() else {
            self.interaction.panel.agent_feed.output.clear();
            self.interaction.panel.agent_feed.scroll = ScrollOffset::default();
            self.interaction.panel.agent_feed.active_task = None;
            self.interaction.panel.agent_feed.current_agent_model = None;
            self.interaction.panel.agent_feed.buffers = EventBuffers::default();
            return;
        };
        let (output, scroll, active_task, current_agent_model, buffers) = {
            let Some(feed) = self
                .interaction
                .panel
                .agent_feed
                .feeds
                .get_mut(selected_index)
            else {
                return;
            };
            let max_offset = feed.output.len().saturating_sub(1);
            feed.scroll = ScrollOffset::of(feed.scroll.inner().min(max_offset));
            (
                feed.output.clone(),
                feed.scroll,
                feed.active_task.clone(),
                feed.current_agent_model.clone(),
                feed.buffers.clone(),
            )
        };
        self.interaction.panel.agent_feed.output = output;
        self.interaction.panel.agent_feed.scroll = scroll;
        self.interaction.panel.agent_feed.active_task = active_task;
        self.interaction.panel.agent_feed.current_agent_model = current_agent_model;
        self.interaction.panel.agent_feed.buffers = buffers;
    }

    /// Transition from plan mode to chat mode and return the plan state.
    pub fn take_plan_state(&mut self) -> Option<PlanModeState> {
        match std::mem::replace(&mut self.interaction.mode, ConversationMode::Chat) {
            ConversationMode::Plan(ps) => Some(ps),
            other => {
                self.interaction.mode = other;
                None
            }
        }
    }

    /// Transition from session selector to conversation screen and return the picker state.
    pub fn take_picker_state(&mut self) -> Option<PickerState> {
        match std::mem::replace(&mut self.interaction.screen, AppScreen::Conversation) {
            AppScreen::SessionSelector(ps) => Some(ps),
            other => {
                self.interaction.screen = other;
                None
            }
        }
    }

    /// Transition from query mode to chat mode and return the query state.
    pub fn take_query_state(&mut self) -> Option<QueryState> {
        match std::mem::replace(&mut self.interaction.mode, ConversationMode::Chat) {
            ConversationMode::Query(qs) => Some(qs),
            other => {
                self.interaction.mode = other;
                None
            }
        }
    }

    /// Return `true` when the top-level screen is the session picker.
    #[allow(dead_code)]
    pub fn is_picker(&self) -> IsPredicate {
        IsPredicate::from(matches!(
            self.interaction.screen,
            AppScreen::SessionSelector(_)
        ))
    }

    /// Return `true` when the conversation is in query mode.
    #[allow(dead_code)]
    pub fn is_query(&self) -> IsPredicate {
        IsPredicate::from(matches!(self.interaction.mode, ConversationMode::Query(_)))
    }

    /// Return `true` when guided-plan mode is currently waiting for compaction.
    #[allow(dead_code)]
    pub fn is_guided_plan_awaiting_compact(&self) -> IsPredicate {
        IsPredicate::from(matches!(
            &self.interaction.mode,
            ConversationMode::GuidedPlan(ui) if ui.guided_awaiting_compact.into()
        ))
    }

    /// Reset visible state when starting a new conversation session.
    pub fn reset_for_new_session(&mut self) {
        self.output.lines.clear();
        self.output.scroll_offset.set(ScrollOffset::of(0));
        self.output.selection = None;
        self.prompt.buffer.clear();
        self.prompt.cursor = 0;
        self.agent.thinking.is_active = false.into();
        self.agent.thinking.label = StatusLabel::new("");
        self.agent.pending_response = None;
        self.agent.pending_tool_call_line_idx = None;
        self.agent.is_turn_complete = false.into();
        self.status.token_totals = augur_domain::domain::types::ProjectTokenTotals::default();
        self.status.last_context = None;
        self.status.reset_usage_on_next_snapshot = true.into();
        self.status.context_window.reset_for_new_session();
        self.interaction.screen = AppScreen::Conversation;
        self.interaction.mode = ConversationMode::Chat;
        self.interaction.panel.ask_panel = None;
        self.interaction.panel.input_focus = InputFocus::Main;
    }

    /// Drain the prompt buffer and return it as a `PromptText`.
    pub fn take_prompt(&mut self) -> PromptText {
        let text: String = self.prompt.buffer.drain(..).collect();
        self.prompt.cursor = 0;
        PromptText::new(text)
    }

    /// Clamp `scroll_offset` to valid bounds: [0, max_offset] where max_offset is
    /// the total display rows minus one - the furthest the user can scroll up
    /// while keeping at least one row of content visible.
    ///
    /// Skips clamping when `last_render_width` is 0 (before the first render):
    /// logical line count is not a reliable proxy for display rows when lines
    /// may wrap.  The first real render will correct the offset via
    /// `recalculate_scroll_for_width_change`.
    fn clamp_output_scroll_offset(&mut self) {
        let width = self.output.last_render_width.get();
        if width == 0 {
            // No reliable display-row count yet; skip clamping.
            // The first render will recalculate the offset correctly.
            return;
        }
        let max_offset = total_output_display_rows(&self.output.lines, width).saturating_sub(1);
        self.output.scroll_offset.set(ScrollOffset::of(
            self.output.scroll_offset.get().inner().min(max_offset),
        ));
    }

    /// Scroll the output pane up by `rows`, clamped to the maximum safe offset.
    pub fn scroll_up(&mut self, rows: Count) {
        self.output.scroll_offset.set(ScrollOffset::of(
            self.output
                .scroll_offset
                .get()
                .inner()
                .saturating_add(rows.inner()),
        ));
        self.clamp_output_scroll_offset();
    }

    /// Scroll the output pane down by `rows`, clamped to zero.
    pub fn scroll_down(&mut self, rows: Count) {
        self.output.scroll_offset.set(ScrollOffset::of(
            self.output
                .scroll_offset
                .get()
                .inner()
                .saturating_sub(rows.inner()),
        ));
    }

    /// Scroll the plan tree panel up by `lines` when in plan mode.
    pub fn plan_scroll_up(&mut self, lines: Count) {
        if let ConversationMode::Plan(ref mut ps) = self.interaction.mode {
            ps.tree_scroll = ScrollOffset::of(ps.tree_scroll.inner().saturating_add(lines.inner()));
        }
    }

    /// Scroll the plan tree panel down by `lines` when in plan mode, clamped to zero.
    pub fn plan_scroll_down(&mut self, lines: Count) {
        if let ConversationMode::Plan(ref mut ps) = self.interaction.mode {
            ps.tree_scroll = ScrollOffset::of(ps.tree_scroll.inner().saturating_sub(lines.inner()));
        }
    }

    /// Clamp `agent_feed.scroll` to valid bounds: [0, max_offset] where max_offset is
    /// the maximum number of lines that can be scrolled up before reaching the top.
    fn clamp_agent_feed_scroll_offset(&mut self) {
        if let Some(index) = self.selected_agent_feed_index() {
            let max_offset = self
                .interaction
                .panel
                .agent_feed
                .feeds
                .get(index)
                .map(|feed| feed.output.len().saturating_sub(1))
                .unwrap_or(0);
            if let Some(feed) = self.interaction.panel.agent_feed.feeds.get_mut(index) {
                feed.scroll = ScrollOffset::of(feed.scroll.inner().min(max_offset));
            }
            self.sync_selected_agent_feed();
        } else {
            let max_offset = self
                .interaction
                .panel
                .agent_feed
                .output
                .len()
                .saturating_sub(1);
            self.interaction.panel.agent_feed.scroll = ScrollOffset::of(
                self.interaction
                    .panel
                    .agent_feed
                    .scroll
                    .inner()
                    .min(max_offset),
            );
        }
    }

    /// Scroll the agent feed panel up by `count` lines.
    pub fn agent_feed_scroll_up(&mut self, count: Count) {
        if let Some(index) = self.selected_agent_feed_index() {
            if let Some(feed) = self.interaction.panel.agent_feed.feeds.get_mut(index) {
                feed.scroll = ScrollOffset::of(feed.scroll.inner().saturating_add(count.inner()));
            }
        } else {
            self.interaction.panel.agent_feed.scroll = ScrollOffset::of(
                self.interaction
                    .panel
                    .agent_feed
                    .scroll
                    .inner()
                    .saturating_add(count.inner()),
            );
        }
        self.clamp_agent_feed_scroll_offset();
    }

    /// Scroll the agent feed panel down by `count` lines, clamped to zero.
    pub fn agent_feed_scroll_down(&mut self, count: Count) {
        if let Some(index) = self.selected_agent_feed_index() {
            if let Some(feed) = self.interaction.panel.agent_feed.feeds.get_mut(index) {
                feed.scroll = ScrollOffset::of(feed.scroll.inner().saturating_sub(count.inner()));
            }
            self.sync_selected_agent_feed();
        } else {
            self.interaction.panel.agent_feed.scroll = ScrollOffset::of(
                self.interaction
                    .panel
                    .agent_feed
                    .scroll
                    .inner()
                    .saturating_sub(count.inner()),
            );
        }
        self.clamp_agent_feed_scroll_offset();
    }
}

/// Sum the display-row counts for all output lines at the given content width.
fn total_output_display_rows(lines: &[OutputLine], width: usize) -> usize {
    lines
        .iter()
        .map(|l| line_display_rows(&rendered_line_text(l), Count::of(width)).inner())
        .sum()
}

#[cfg(test)]
#[path = "../../../tests/domain/tui_state/lifecycle.tests.rs"]
mod tests;
