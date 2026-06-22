//! Event-selection and event-application helpers for the TUI runtime loop.

use super::super::guided_plan::{
    apply_guided_plan_actions, handle_guided_plan_event, supervisor_event_to_feed,
};
use super::super::{CHARS_PER_TICK, EventOutcome, TuiHandles, TuiStreams};
use super::maybe_finish_guided_plan_compaction;
use super::terminal::handle_terminal_event;
use crate::actors::tui::assistant::output_buf::{drain_char_buf, handle_agent_output};
use crate::actors::tui::assistant::picker::handle_picker_event;
use crate::actors::tui::assistant::plan_view::{
    handle_query_request, handle_supervisor_event, recv_supervisor,
};
use crate::domain::tui_input::{apply_agent_feed_output, apply_agent_output, apply_ask_output};
use crate::domain::tui_state::AppState;
use augur_core::domain::deterministic_orchestrator::{
    DeterministicOrchestratorEvent, NormalizedSignal,
};
use augur_domain::domain::string_newtypes::OutputText;
use augur_domain::domain::string_newtypes::ToolCallId;
use augur_domain::domain::types::{AgentFeedOutput, AgentOutput, FeedEntry, FeedId};
use futures_util::StreamExt;
use std::ops::ControlFlow;

/// Wait for the next runtime event and apply it to TUI state.
pub(super) async fn select_next_event(
    state: &mut AppState,
    streams: TuiStreams<'_>,
    handles: &TuiHandles<'_>,
) -> EventOutcome {
    let can_tick = should_tick(state, streams.char_buf);
    tokio::select! {
        maybe_event = streams.event_stream.next() => {
            handle_input_event(state, maybe_event, handles).await
        }
        agent_out = streams.channels.output_rx.recv() => {
            handle_agent_output_event(
                state,
                agent_out,
                AgentOutputEventContext::new(streams.char_buf, handles),
            )
        }
        query_req = streams.channels.query_rx.recv() => {
            handle_query_event(state, query_req)
        }
        supervisor_ev = recv_supervisor(streams.channels.background.supervisor_rx) => {
            handle_supervisor_update(state, supervisor_ev)
        }
        plan_ev = streams.channels.guided_plan_rx.recv() => {
            handle_guided_plan_update(state, plan_ev, handles)
        }
        _ = streams.ticker.tick(), if can_tick => {
            handle_tick(state, streams.char_buf)
        }
        ask_ev = streams.channels.ask_output_rx.recv() => {
            handle_ask_output_event(state, ask_ev)
        }
        feed_ev = streams.channels.background.agent_feed_rx.recv() => {
            handle_agent_feed_event(state, feed_ev, handles.tools.logger)
        }
        orch_ev = streams.channels.background.orchestrator_event_rx.recv() => {
            handle_orchestrator_event(state, orch_ev, handles.tools.logger)
        }
        _ = streams.snapshot.ticker.tick() => {
            handle_snapshot_tick(state, streams.snapshot.token_tracker).await
        }
    }
}

struct AgentOutputEventContext<'buf, 'ctx, 'handles> {
    char_buf: &'buf mut augur_domain::domain::string_newtypes::OutputText,
    handles: &'ctx TuiHandles<'handles>,
}

impl<'buf, 'ctx, 'handles> AgentOutputEventContext<'buf, 'ctx, 'handles> {
    fn new(
        char_buf: &'buf mut augur_domain::domain::string_newtypes::OutputText,
        handles: &'ctx TuiHandles<'handles>,
    ) -> Self {
        Self { char_buf, handles }
    }
}

fn should_tick(
    state: &AppState,
    char_buf: &augur_domain::domain::string_newtypes::OutputText,
) -> bool {
    state.agent.thinking.is_active.into()
        || bool::from(state.any_agent_feed_active())
        || !char_buf.is_empty()
        || state.status.context_window.backoff_until.is_some()
        || ask_panel_is_thinking(state)
}

async fn handle_input_event(
    state: &mut AppState,
    maybe_event: Option<Result<crossterm::event::Event, std::io::Error>>,
    handles: &TuiHandles<'_>,
) -> EventOutcome {
    if matches!(
        state.interaction.screen,
        crate::domain::tui_state::AppScreen::SessionSelector(_)
    ) {
        return picker_outcome(handle_picker_event(state, maybe_event, handles).await);
    }
    handle_terminal_event(state, maybe_event, handles).await
}

fn picker_outcome(outcome: ControlFlow<()>) -> EventOutcome {
    if matches!(outcome, ControlFlow::Break(())) {
        EventOutcome::Quit
    } else {
        EventOutcome::Redraw
    }
}

fn handle_agent_output_event(
    state: &mut AppState,
    agent_out: Result<AgentOutput, tokio::sync::broadcast::error::RecvError>,
    event_ctx: AgentOutputEventContext<'_, '_, '_>,
) -> EventOutcome {
    let is_compaction_done =
        matches!(&agent_out, Ok(AgentOutput::CompactionComplete { .. })).then_some(());
    let quit = if matches!(
        state.interaction.screen,
        crate::domain::tui_state::AppScreen::SessionSelector(_)
    ) {
        handle_picker_agent_output(state, agent_out)
    } else if matches!(&agent_out, Ok(AgentOutput::Token(_))) {
        let _ = handle_agent_output(state, agent_out, event_ctx.char_buf);
        return EventOutcome::NoOp;
    } else {
        handle_agent_output(state, agent_out, event_ctx.char_buf)
    };
    maybe_finish_guided_plan_compaction(state, is_compaction_done, event_ctx.handles);
    picker_outcome(quit)
}

fn handle_picker_agent_output(
    state: &mut AppState,
    agent_out: Result<AgentOutput, tokio::sync::broadcast::error::RecvError>,
) -> ControlFlow<()> {
    if let Ok(output) = agent_out
        && matches!(output, AgentOutput::ModelsAvailable(_))
    {
        apply_agent_output(state, output);
    }
    ControlFlow::Continue(())
}

fn handle_query_event(
    state: &mut AppState,
    query_req: Option<augur_domain::tools::builtin::query_user::QueryUserRequest>,
) -> EventOutcome {
    handle_query_request(state, query_req);
    EventOutcome::Redraw
}

fn handle_supervisor_update(
    state: &mut AppState,
    supervisor_ev: Option<
        Result<
            augur_domain::domain::types::SupervisorEvent,
            tokio::sync::broadcast::error::RecvError,
        >,
    >,
) -> EventOutcome {
    let Some(Ok(event)) = supervisor_ev else {
        return EventOutcome::NoOp;
    };
    let feed_output = supervisor_event_to_feed(&event);
    handle_supervisor_event(state, event);
    if let Some(output) = feed_output {
        apply_agent_feed_output(
            state,
            FeedEntry {
                feed_id: FeedId::Agent(ToolCallId::from("supervisor")),
                output,
            },
        );
    }
    EventOutcome::Redraw
}

fn handle_guided_plan_update(
    state: &mut AppState,
    plan_ev: Result<
        augur_domain::domain::guided_plan::GuidedPlanEvent,
        tokio::sync::broadcast::error::RecvError,
    >,
    handles: &TuiHandles<'_>,
) -> EventOutcome {
    let Ok(event) = plan_ev else {
        return EventOutcome::NoOp;
    };
    apply_guided_plan_actions(state, &event, handles);
    handle_guided_plan_event(state, event);
    EventOutcome::Redraw
}

fn handle_tick(
    state: &mut AppState,
    char_buf: &mut augur_domain::domain::string_newtypes::OutputText,
) -> EventOutcome {
    let spinner_active = state.agent.thinking.is_active.into()
        || bool::from(state.any_agent_feed_active())
        || ask_panel_is_thinking(state);
    if spinner_active {
        state.agent.thinking.spinner_tick = state.agent.thinking.spinner_tick.wrapping_add(1);
    }
    drain_char_buf(
        state,
        char_buf,
        augur_domain::domain::newtypes::Count::of(CHARS_PER_TICK),
    );
    EventOutcome::Redraw
}

/// True when the ask panel exists and is currently waiting for a response.
fn ask_panel_is_thinking(state: &AppState) -> bool {
    state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .map(|p| bool::from(p.thinking))
        .unwrap_or(false)
}

fn handle_ask_output_event(
    state: &mut AppState,
    ask_ev: Result<AgentOutput, tokio::sync::broadcast::error::RecvError>,
) -> EventOutcome {
    let Ok(output) = ask_ev else {
        return EventOutcome::NoOp;
    };
    tracing::info!(
        output = ?output,
        ask_panel_present = state.interaction.panel.ask_panel.is_some(),
        "tui.runtime.ask_event"
    );
    apply_ask_output(state, output);
    EventOutcome::Redraw
}

fn handle_agent_feed_event(
    state: &mut AppState,
    feed_ev: Option<FeedEntry>,
    logger: &augur_core::actors::LoggerHandle,
) -> EventOutcome {
    let Some(event) = feed_ev else {
        return EventOutcome::NoOp;
    };
    log_agent_feed_event(state, &event);
    push_pipeline_failure_message(state, &event.output);
    let log_line = format_agent_feed_log_line(&event.output);
    logger.log_line(OutputText::from("agent"), OutputText::from(log_line));
    apply_agent_feed_output(state, event);
    EventOutcome::Redraw
}

fn log_agent_feed_event(state: &AppState, event: &FeedEntry) {
    tracing::info!(
        feed_id = ?event.feed_id,
        event = ?event.output,
        secondary_view = ?state.interaction.panel.secondary_view,
        input_focus = ?state.interaction.panel.input_focus,
        "tui.runtime.agent_feed_event"
    );
}

fn push_pipeline_failure_message(state: &mut AppState, output: &AgentFeedOutput) {
    if let AgentFeedOutput::TaskFailed { name, reason } = output {
        state.push_system_message(format!("[pipeline] agent {} failed: {}", name, reason).as_str());
    }
}

fn format_agent_feed_log_line(output: &AgentFeedOutput) -> String {
    format_task_lifecycle_log_line(output)
        .or_else(|| format_status_log_line(output))
        .or_else(|| format_control_log_line(output))
        .unwrap_or_else(|| "[agent] clear".to_string())
}

fn format_task_lifecycle_log_line(output: &AgentFeedOutput) -> Option<String> {
    match output {
        AgentFeedOutput::TaskStarted { name, .. } => Some(format!("[agent:{}] started", name)),
        AgentFeedOutput::TaskCompleted { name } => Some(format!("[agent:{}] completed", name)),
        AgentFeedOutput::TaskFailed { name, reason } => {
            Some(format!("[agent:{}] failed: {}", name, reason))
        }
        _ => None,
    }
}

fn format_status_log_line(output: &AgentFeedOutput) -> Option<String> {
    match output {
        AgentFeedOutput::StatusLine(text) => Some(format!("[agent] status: {}", text)),
        AgentFeedOutput::ToolEventLine(text) => Some(format!("[agent] tool: {}", text)),
        _ => None,
    }
}

fn format_control_log_line(output: &AgentFeedOutput) -> Option<String> {
    match output {
        AgentFeedOutput::MessageBreak => Some("[agent] message-break".to_string()),
        AgentFeedOutput::Clear => Some("[agent] clear".to_string()),
        _ => None,
    }
}

/// Formats a `DeterministicOrchestratorEvent` as a system message and pushes it to TUI state.
///
/// Inputs:
/// - `state`: mutable TUI application state.
/// - `recv_result`: result from a `broadcast::Receiver::recv()` call.
/// - `logger`: logger handle for writing the event to the session JSONL log.
///
/// Returns `NoOp` on lagged or closed channel errors; `Redraw` on success.
fn handle_orchestrator_event(
    state: &mut AppState,
    recv_result: Result<DeterministicOrchestratorEvent, tokio::sync::broadcast::error::RecvError>,
    logger: &augur_core::actors::LoggerHandle,
) -> EventOutcome {
    let Ok(event) = recv_result else {
        return EventOutcome::NoOp;
    };
    tracing::info!(event = ?event, "tui.runtime.orchestrator_event");
    let message = format_orchestrator_event(&event);
    state.push_system_message(message.as_str());
    logger.log_line(OutputText::from("system"), OutputText::from(message));
    EventOutcome::Redraw
}

/// Converts a `DeterministicOrchestratorEvent` to a human-readable system message string.
fn format_orchestrator_event(event: &DeterministicOrchestratorEvent) -> String {
    format_started_orchestrator_event(event)
        .or_else(|| format_progressed_orchestrator_event(event))
        .or_else(|| format_rerun_orchestrator_event(event))
        .or_else(|| format_backtracked_orchestrator_event(event))
        .or_else(|| format_halted_orchestrator_event(event))
        .or_else(|| format_completed_orchestrator_event(event))
        .unwrap_or_else(|| "[pipeline] completed".to_string())
}

fn format_started_orchestrator_event(event: &DeterministicOrchestratorEvent) -> Option<String> {
    if let DeterministicOrchestratorEvent::Started { first_step_id } = event {
        Some(format_started_event(first_step_id.as_ref()))
    } else {
        None
    }
}

fn format_progressed_orchestrator_event(event: &DeterministicOrchestratorEvent) -> Option<String> {
    if let DeterministicOrchestratorEvent::StepProgressed {
        step_id,
        signal,
        agent_name,
    } = event
    {
        Some(format_step_progressed_event(
            step_id,
            signal,
            agent_name.as_deref(),
        ))
    } else {
        None
    }
}

fn format_rerun_orchestrator_event(event: &DeterministicOrchestratorEvent) -> Option<String> {
    if let DeterministicOrchestratorEvent::RerunScheduled { step_id } = event {
        Some(format!("[pipeline] step {step_id} - scheduled for rerun"))
    } else {
        None
    }
}

fn format_backtracked_orchestrator_event(event: &DeterministicOrchestratorEvent) -> Option<String> {
    if let DeterministicOrchestratorEvent::Backtracked {
        from_step_id,
        to_step_id,
    } = event
    {
        Some(format!(
            "[pipeline] backtracking from {from_step_id} to {to_step_id}"
        ))
    } else {
        None
    }
}

fn format_halted_orchestrator_event(event: &DeterministicOrchestratorEvent) -> Option<String> {
    if let DeterministicOrchestratorEvent::Halted { step_id } = event {
        Some(format!("[pipeline] halted at step {step_id}"))
    } else {
        None
    }
}

fn format_completed_orchestrator_event(event: &DeterministicOrchestratorEvent) -> Option<String> {
    if let DeterministicOrchestratorEvent::Completed = event {
        Some("[pipeline] completed".to_string())
    } else {
        None
    }
}

/// Format a `Started` event as a human-readable message.
fn format_started_event(first_step_id: Option<&impl std::fmt::Display>) -> String {
    match first_step_id {
        Some(id) => format!("[pipeline] started - first step: {id}"),
        None => "[pipeline] started - no steps found".to_string(),
    }
}

/// Format a `StepProgressed` event as a human-readable message.
fn format_step_progressed_event(
    step_id: &impl std::fmt::Display,
    signal: &NormalizedSignal,
    agent_name: Option<&str>,
) -> String {
    let label = normalize_signal_label(signal);
    match agent_name {
        Some(name) => format!("[pipeline] step {step_id} > {name} - {label}"),
        None => format!("[pipeline] step {step_id} - {label}"),
    }
}

/// Map a `NormalizedSignal` to its display label.
fn normalize_signal_label(signal: &NormalizedSignal) -> &'static str {
    match signal {
        NormalizedSignal::Advance => "pass",
        NormalizedSignal::Hold => "hold",
        NormalizedSignal::NeedsRevision => "needs-revision",
    }
}

async fn handle_snapshot_tick(
    state: &mut AppState,
    token_tracker: &augur_core::actors::token_tracker::TokenTrackerHandle,
) -> EventOutcome {
    if state.status.reset_usage_on_next_snapshot.into() {
        token_tracker.reset_totals();
    }
    let totals = token_tracker.snapshot().await;
    let display_totals = if state.status.reset_usage_on_next_snapshot.into() {
        state.status.token_totals_baseline = totals.clone();
        state.status.reset_usage_on_next_snapshot = false.into();
        augur_domain::domain::types::ProjectTokenTotals::default()
    } else {
        totals_since_baseline(&totals, &state.status.token_totals_baseline)
    };
    apply_agent_output(
        state,
        augur_domain::domain::types::AgentOutput::UsageSnapshot(display_totals),
    );
    EventOutcome::Redraw
}

fn totals_since_baseline(
    current: &augur_domain::domain::types::ProjectTokenTotals,
    baseline: &augur_domain::domain::types::ProjectTokenTotals,
) -> augur_domain::domain::types::ProjectTokenTotals {
    augur_domain::domain::types::ProjectTokenTotals {
        tokens_in: augur_domain::domain::TokenCount::of(
            (*current.tokens_in).saturating_sub(*baseline.tokens_in),
        ),
        tokens_out: augur_domain::domain::TokenCount::of(
            (*current.tokens_out).saturating_sub(*baseline.tokens_out),
        ),
        tokens_cached: augur_domain::domain::TokenCount::of(
            (*current.tokens_cached).saturating_sub(*baseline.tokens_cached),
        ),
        cache_write_tokens: augur_domain::domain::TokenCount::of(
            (*current.cache_write_tokens).saturating_sub(*baseline.cache_write_tokens),
        ),
        cost_usd: (f64::from(current.cost_usd) - f64::from(baseline.cost_usd))
            .max(0.0)
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tui_state::{AppScreen, AppState};
    use augur_core::actors::logger::logger_actor::spawn as spawn_logger;
    use augur_domain::domain::string_newtypes::{EndpointName, StringNewtype, TaskName};
    use augur_domain::string_newtypes::WorkflowStepId;

    fn conversation_state() -> AppState {
        AppState::new(EndpointName::new("openrouter"), AppScreen::Conversation)
    }

    fn logger_handle() -> augur_core::actors::LoggerHandle {
        let temp = tempfile::tempdir().expect("tempdir");
        let (_join, logger) = spawn_logger(temp.path().to_path_buf());
        std::mem::forget(temp);
        logger
    }

    #[tokio::test]
    async fn orchestrator_completed_does_not_clear_active_background_task() {
        let mut state = conversation_state();
        state.interaction.panel.agent_feed.active_task = Some(TaskName::new("running-task"));
        let logger = logger_handle();
        let event = Ok(DeterministicOrchestratorEvent::Completed);

        let outcome = handle_orchestrator_event(&mut state, event, &logger);

        assert!(matches!(outcome, EventOutcome::Redraw));
        assert!(
            state.interaction.panel.agent_feed.active_task.is_some(),
            "background task must remain active until TaskCompleted/TaskFailed signal"
        );
    }

    #[tokio::test]
    async fn orchestrator_halted_does_not_clear_active_background_task() {
        let mut state = conversation_state();
        state.interaction.panel.agent_feed.active_task = Some(TaskName::new("running-task"));
        let logger = logger_handle();
        let event = Ok(DeterministicOrchestratorEvent::Halted {
            step_id: WorkflowStepId::from("implement-behavior"),
        });

        let outcome = handle_orchestrator_event(&mut state, event, &logger);

        assert!(matches!(outcome, EventOutcome::Redraw));
        assert!(
            state.interaction.panel.agent_feed.active_task.is_some(),
            "background task must remain active until TaskCompleted/TaskFailed signal"
        );
    }
}
