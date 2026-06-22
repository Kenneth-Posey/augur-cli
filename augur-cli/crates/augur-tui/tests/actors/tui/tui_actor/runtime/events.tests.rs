use crate::domain::string_newtypes::{
    AgentName, EndpointName, OutputText, PromptText, StringNewtype, TaskName,
};
use crate::domain::traits::ChatProvider;
use crate::domain::tui_state::{AppScreen, AppState};
use crate::domain::types::{AgentFeedOutput, AgentOutput, FeedEntry, FeedId, SupervisorEvent};
use crate::domain::{DeterministicOrchestratorEvent, NormalizedSignal, WorkflowStepId};
use crate::persistence::types::MessageRecord;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn conversation_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

struct RecordingCompactProvider {
    compact_calls: Arc<Mutex<usize>>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
}

impl RecordingCompactProvider {
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(8);
        Self {
            compact_calls: Arc::new(Mutex::new(0)),
            output_tx,
        }
    }

    fn compact_call_count(&self) -> usize {
        *self.compact_calls.lock().expect("compact count lock")
    }
}

impl ChatProvider for RecordingCompactProvider {
    fn submit(&self, _prompt: PromptText, _endpoint: Option<EndpointName>) {}

    fn interrupt(&self) {}

    fn shutdown(&self) {}

    fn restore(&self, _records: Vec<MessageRecord>) {}

    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    fn compact(&self) {
        *self.compact_calls.lock().expect("compact count lock") += 1;
    }
}

struct TestRigCoreHandles {
    command: crate::actors::command::handle::CommandHandle,
    session: crate::actors::SessionHandle,
    persistence: crate::persistence::handle::PersistenceHandle,
}

struct TestRigToolHandles {
    scanner: crate::actors::file_scanner::FileScannerHandle,
    guided_plan: crate::actors::guided_plan::GuidedPlanHandle,
    ask: crate::actors::ask::AskHandle,
    logger: crate::actors::LoggerHandle,
}

struct TestRigResources {
    _persistence_dir: tempfile::TempDir,
    _scanner_join: tokio::task::JoinHandle<()>,
    _ask_dir: tempfile::TempDir,
    _logger_join: tokio::task::JoinHandle<()>,
}

struct TestRig {
    provider: RecordingCompactProvider,
    core: TestRigCoreHandles,
    tools: TestRigToolHandles,
    _resources: TestRigResources,
}

impl TestRig {
    async fn new() -> Self {
        let command = crate::actors::command::command_actor::build(&[]);
        let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
        let persistence_dir = tempfile::tempdir().expect("tempdir");
        let persistence =
            crate::persistence::handle::PersistenceHandle::new(persistence_dir.path().to_owned());
        let (scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
        let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
        let (ask, ask_dir) = crate::tests::helpers::fake_ask::make_ask_handle().await;
        let (logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
        Self {
            provider: RecordingCompactProvider::new(),
            core: TestRigCoreHandles {
                command,
                session,
                persistence,
            },
            tools: TestRigToolHandles {
                scanner,
                guided_plan,
                ask,
                logger,
            },
            _resources: TestRigResources {
                _persistence_dir: persistence_dir,
                _scanner_join: scanner_join,
                _ask_dir: ask_dir,
                _logger_join: logger_join,
            },
        }
    }

    fn handles(&self) -> crate::actors::tui::tui_actor::TuiHandles<'_> {
        let (_catalog_manager_join, catalog_manager) =
            crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
        crate::actors::tui::tui_actor::TuiHandles {
            agent: &self.provider,
            session: &self.core.session,
            persistence: &self.core.persistence,
            tools: crate::actors::tui::tui_actor::TuiToolHandles {
                command: &self.core.command,
                file_scanner: &self.tools.scanner,
                guided_plan: &self.tools.guided_plan,
                ask: &self.tools.ask,
                logger: &self.tools.logger,
            },
            work: crate::actors::tui::tui_actor::TuiWorkHandles {
                orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
                catalog_manager,
            },
        }
    }
}

/// Verifies that the runtime keeps the ticker branch enabled while backoff is active, even with no spinner or buffered text.
#[test]
fn should_tick_returns_true_while_backoff_is_active() {
    let mut state = conversation_state();
    state.status.context_window.backoff_until = Some(Instant::now() + Duration::from_secs(5));

    assert!(
        super::should_tick(&state, &OutputText::new("")),
        "active backoff must keep runtime ticking so the countdown can refresh"
    );
}

/// Verifies that the runtime disables ticker work only when there is no spinner, no buffered text, and no active backoff.
#[test]
fn should_tick_returns_false_when_runtime_is_fully_idle() {
    let state = conversation_state();

    assert!(
        !super::should_tick(&state, &OutputText::new("")),
        "idle runtime with no backoff should not tick"
    );
}

/// Verifies that `handle_tick` advances the spinner, drains buffered output text, and requests a redraw.
#[test]
fn handle_tick_advances_spinner_drains_buffer_and_returns_redraw() {
    let mut state = conversation_state();
    state.agent.thinking.is_active = true;
    state.agent.thinking.spinner_tick = 7;
    let mut char_buf = OutputText::new("abcdefghi");

    let outcome = super::handle_tick(&mut state, &mut char_buf);

    assert!(matches!(outcome, super::EventOutcome::Redraw));
    assert_eq!(
        state.agent.thinking.spinner_tick, 8,
        "active thinking state must advance the spinner on each tick"
    );
    assert_eq!(
        state.output.lines[0].text.as_str(),
        "abcdef",
        "tick must drain exactly CHARS_PER_TICK characters into the output pane"
    );
    assert_eq!(
        char_buf.as_str(),
        "ghi",
        "tick must leave any remaining buffered characters queued for later ticks"
    );
}

/// Verifies that the runtime keeps ticking when `agent_feed.active_task` is set, even when
/// `thinking.is_active` is false - required for the spinner to animate during agent-feed tasks.
#[test]
fn should_tick_returns_true_when_agent_feed_has_active_task() {
    let mut state = conversation_state();
    state.agent.thinking.is_active = false;
    state.interaction.panel.agent_feed.active_task = Some(TaskName::new("some-task"));

    assert!(
        super::should_tick(&state, &OutputText::new("")),
        "active agent-feed task must keep runtime ticking so spinner can animate"
    );
}

/// Verifies that `handle_tick` advances `spinner_tick` when only `agent_feed.active_task` is
/// set and `thinking.is_active` is false - the spinner must not be frozen at frame 0.
#[test]
fn handle_tick_advances_spinner_when_agent_feed_active_task_present() {
    let mut state = conversation_state();
    state.agent.thinking.is_active = false;
    state.agent.thinking.spinner_tick = 3;
    state.interaction.panel.agent_feed.active_task = Some(TaskName::new("some-task"));
    let mut char_buf = OutputText::new("");

    let outcome = super::handle_tick(&mut state, &mut char_buf);

    assert!(matches!(outcome, super::EventOutcome::Redraw));
    assert_eq!(
        state.agent.thinking.spinner_tick, 4,
        "agent-feed active task must advance the spinner tick even when thinking.is_active is false"
    );
}

/// Verifies that `should_tick` returns true when `ask_panel.thinking` is true,
/// even when main thinking and agent feed are both inactive.
///
/// The ask panel spinner must animate while the ask actor is processing a request.
#[test]
fn should_tick_returns_true_when_ask_panel_thinking() {
    use crate::domain::tui_state::AskPanelState;

    let mut state = conversation_state();
    state.agent.thinking.is_active = false;
    // agent_feed.active_task is None by default.
    let ask = AskPanelState {
        thinking: true,
        ..Default::default()
    };
    state.interaction.panel.ask_panel = Some(ask);

    assert!(
        super::should_tick(&state, &OutputText::new("")),
        "ask panel thinking must keep runtime ticking so the spinner can animate"
    );
}

/// Verifies that `handle_tick` advances `spinner_tick` when only `ask_panel.thinking`
/// is true and both main thinking and agent feed are inactive.
///
/// The inline spinner in the ask panel title row depends on `spinner_tick` advancing.
#[test]
fn handle_tick_advances_spinner_when_ask_panel_thinking() {
    use crate::domain::tui_state::AskPanelState;

    let mut state = conversation_state();
    state.agent.thinking.is_active = false;
    state.agent.thinking.spinner_tick = 5;
    // agent_feed.active_task is None by default.
    let ask = AskPanelState {
        thinking: true,
        ..Default::default()
    };
    state.interaction.panel.ask_panel = Some(ask);
    let mut char_buf = OutputText::new("");

    let outcome = super::handle_tick(&mut state, &mut char_buf);

    assert!(matches!(outcome, super::EventOutcome::Redraw));
    assert_eq!(
        state.agent.thinking.spinner_tick, 6,
        "ask panel thinking must advance spinner_tick even when main thinking is inactive"
    );
}

// ── format_orchestrator_event ─────────────────────────────────────────────────

/// Verifies that `format_orchestrator_event` renders `Started` with a step id correctly.
#[test]
fn format_orchestrator_event_started_with_step_id() {
    // Given: a Started event carrying a first_step_id
    let event = DeterministicOrchestratorEvent::Started {
        first_step_id: Some(WorkflowStepId::from("design-requirements")),
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message includes the step id
    assert_eq!(
        msg, "[pipeline] started - first step: design-requirements",
        "Started with step id must include the step id in the formatted message"
    );
}

/// Verifies that `format_orchestrator_event` renders `Started` with no step id correctly.
#[test]
fn format_orchestrator_event_started_without_step_id() {
    // Given: a Started event with no first_step_id
    let event = DeterministicOrchestratorEvent::Started {
        first_step_id: None,
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message says no steps found
    assert_eq!(
        msg, "[pipeline] started - no steps found",
        "Started with no step id must produce the 'no steps found' message"
    );
}

/// Verifies that `format_orchestrator_event` renders `StepProgressed` with Advance and an agent name.
#[test]
fn format_orchestrator_event_step_progressed_advance_with_agent_name() {
    // Given: a StepProgressed/Advance event with an agent name
    let event = DeterministicOrchestratorEvent::StepProgressed {
        step_id: WorkflowStepId::from("implement-behavior"),
        signal: NormalizedSignal::Advance,
        agent_name: Some("behavior-builder".to_string()),
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message includes step id, agent name, and pass signal
    assert_eq!(
        msg, "[pipeline] step implement-behavior > behavior-builder - pass",
        "Advance with agent name must render step, agent name, and 'pass'"
    );
}

/// Verifies that `format_orchestrator_event` renders `StepProgressed` with Advance and no agent name.
#[test]
fn format_orchestrator_event_step_progressed_advance_without_agent_name() {
    // Given: a StepProgressed/Advance event with no agent name
    let event = DeterministicOrchestratorEvent::StepProgressed {
        step_id: WorkflowStepId::from("implement-behavior"),
        signal: NormalizedSignal::Advance,
        agent_name: None,
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message includes step id and pass signal without an agent name
    assert_eq!(
        msg, "[pipeline] step implement-behavior - pass",
        "Advance without agent name must render step and 'pass' only"
    );
}

/// Verifies that `format_orchestrator_event` renders `StepProgressed` with Hold and an agent name.
#[test]
fn format_orchestrator_event_step_progressed_hold_with_agent_name() {
    // Given: a StepProgressed/Hold event with an agent name
    let event = DeterministicOrchestratorEvent::StepProgressed {
        step_id: WorkflowStepId::from("design-requirements"),
        signal: NormalizedSignal::Hold,
        agent_name: Some("design-requirements-reviewer".to_string()),
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message includes step id, agent name, and hold signal
    assert_eq!(
        msg, "[pipeline] step design-requirements > design-requirements-reviewer - hold",
        "Hold with agent name must render step, agent name, and 'hold'"
    );
}

/// Verifies that `format_orchestrator_event` renders `StepProgressed` with Hold and no agent name.
#[test]
fn format_orchestrator_event_step_progressed_hold_without_agent_name() {
    // Given: a StepProgressed/Hold event with no agent name
    let event = DeterministicOrchestratorEvent::StepProgressed {
        step_id: WorkflowStepId::from("design-requirements"),
        signal: NormalizedSignal::Hold,
        agent_name: None,
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message says hold without agent name
    assert_eq!(
        msg, "[pipeline] step design-requirements - hold",
        "Hold without agent name must render step and 'hold' only"
    );
}

/// Verifies that `format_orchestrator_event` renders `StepProgressed` with NeedsRevision and an agent name.
#[test]
fn format_orchestrator_event_step_progressed_needs_revision_with_agent_name() {
    // Given: a StepProgressed/NeedsRevision event with an agent name
    let event = DeterministicOrchestratorEvent::StepProgressed {
        step_id: WorkflowStepId::from("plan-builder"),
        signal: NormalizedSignal::NeedsRevision,
        agent_name: Some("plan-evaluator".to_string()),
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message includes step id, agent name, and needs-revision signal
    assert_eq!(
        msg, "[pipeline] step plan-builder > plan-evaluator - needs-revision",
        "NeedsRevision with agent name must render step, agent name, and 'needs-revision'"
    );
}

/// Verifies that `format_orchestrator_event` renders `StepProgressed` with NeedsRevision and no agent name.
#[test]
fn format_orchestrator_event_step_progressed_needs_revision_without_agent_name() {
    // Given: a StepProgressed/NeedsRevision event with no agent name
    let event = DeterministicOrchestratorEvent::StepProgressed {
        step_id: WorkflowStepId::from("plan-builder"),
        signal: NormalizedSignal::NeedsRevision,
        agent_name: None,
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message says needs-revision without agent name
    assert_eq!(
        msg, "[pipeline] step plan-builder - needs-revision",
        "NeedsRevision without agent name must render step and 'needs-revision' only"
    );
}

/// Verifies that `format_orchestrator_event` renders `RerunScheduled` correctly.
#[test]
fn format_orchestrator_event_rerun_scheduled() {
    // Given: a RerunScheduled event
    let event = DeterministicOrchestratorEvent::RerunScheduled {
        step_id: WorkflowStepId::from("implement-behavior"),
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message says scheduled for rerun
    assert_eq!(
        msg, "[pipeline] step implement-behavior - scheduled for rerun",
        "RerunScheduled must render step and 'scheduled for rerun'"
    );
}

/// Verifies that `format_orchestrator_event` renders `Backtracked` correctly.
#[test]
fn format_orchestrator_event_backtracked() {
    // Given: a Backtracked event
    let event = DeterministicOrchestratorEvent::Backtracked {
        from_step_id: WorkflowStepId::from("implement-behavior"),
        to_step_id: WorkflowStepId::from("design-requirements"),
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message shows the from and to step ids
    assert_eq!(
        msg, "[pipeline] backtracking from implement-behavior to design-requirements",
        "Backtracked must render from_step_id and to_step_id"
    );
}

/// Verifies that `format_orchestrator_event` renders `Halted` correctly.
#[test]
fn format_orchestrator_event_halted() {
    // Given: a Halted event
    let event = DeterministicOrchestratorEvent::Halted {
        step_id: WorkflowStepId::from("implement-behavior"),
    };

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message says halted at the step
    assert_eq!(
        msg, "[pipeline] halted at step implement-behavior",
        "Halted must render the step id in the message"
    );
}

/// Verifies that `format_orchestrator_event` renders `Completed` correctly.
#[test]
fn format_orchestrator_event_completed() {
    // Given: a Completed event
    let event = DeterministicOrchestratorEvent::Completed;

    // When: the event is formatted
    let msg = super::format_orchestrator_event(&event);

    // Then: the message says completed
    assert_eq!(
        msg, "[pipeline] completed",
        "Completed must render exactly '[pipeline] completed'"
    );
}

// ── handle_query_event ────────────────────────────────────────────────────────

/// Verifies that `handle_query_event` returns `Redraw` when the channel is closed (None).
#[test]
fn handle_query_event_returns_redraw_on_none() {
    // Given: a conversation state and a closed query channel (None)
    let mut state = conversation_state();

    // When: the query event handler receives None
    let outcome = super::handle_query_event(&mut state, None);

    // Then: it returns Redraw (query_request with None is a no-op that still redraws)
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_query_event must return Redraw even when the channel is closed"
    );
}

// ── handle_supervisor_update ──────────────────────────────────────────────────

/// Verifies that `handle_supervisor_update` returns `NoOp` when the channel is closed (None).
#[test]
fn handle_supervisor_update_returns_noop_on_none() {
    // Given: a conversation state and a closed supervisor channel (None)
    let mut state = conversation_state();

    // When: the supervisor update handler receives None
    let outcome = super::handle_supervisor_update(&mut state, None);

    // Then: it returns NoOp because there is nothing to apply
    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "handle_supervisor_update must return NoOp when supervisor channel returns None"
    );
}

/// Verifies that `handle_supervisor_update` returns `NoOp` on a lagged broadcast error.
#[test]
fn handle_supervisor_update_returns_noop_on_lagged_error() {
    // Given: a conversation state and a lagged broadcast error
    let mut state = conversation_state();
    let lagged = Some(Err(tokio::sync::broadcast::error::RecvError::Lagged(1)));

    // When: the supervisor update handler receives a lagged error
    let outcome = super::handle_supervisor_update(&mut state, lagged);

    // Then: it returns NoOp because lagged messages are ignored
    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "handle_supervisor_update must return NoOp on lagged broadcast error"
    );
}

/// Verifies that `handle_supervisor_update` returns `Redraw` when a valid event arrives.
#[test]
fn handle_supervisor_update_returns_redraw_on_valid_event() {
    // Given: a conversation state and a valid ExecutionComplete supervisor event
    let mut state = conversation_state();
    let event = Some(Ok(SupervisorEvent::ExecutionComplete));

    // When: the supervisor update handler receives the event
    let outcome = super::handle_supervisor_update(&mut state, event);

    // Then: it returns Redraw because the TUI state may have changed
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_supervisor_update must return Redraw when a valid supervisor event is applied"
    );
}

// ── handle_ask_output_event ───────────────────────────────────────────────────

/// Verifies that `handle_ask_output_event` returns `NoOp` on a lagged broadcast error.
#[test]
fn handle_ask_output_event_returns_noop_on_error() {
    // Given: a conversation state and a lagged broadcast error
    let mut state = conversation_state();
    let err = Err(tokio::sync::broadcast::error::RecvError::Lagged(1));

    // When: the ask output event handler receives an error
    let outcome = super::handle_ask_output_event(&mut state, err);

    // Then: it returns NoOp because errors are silently ignored
    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "handle_ask_output_event must return NoOp on broadcast error"
    );
}

/// Verifies that `handle_ask_output_event` returns `Redraw` when a valid AgentOutput arrives.
#[test]
fn handle_ask_output_event_returns_redraw_on_valid_output() {
    // Given: a conversation state and a valid Done output for the ask panel
    let mut state = conversation_state();
    let output = Ok(AgentOutput::Done);

    // When: the ask output event handler receives the output
    let outcome = super::handle_ask_output_event(&mut state, output);

    // Then: it returns Redraw so the ask panel can update
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_ask_output_event must return Redraw when a valid AgentOutput is applied"
    );
}

// ── handle_agent_feed_event ───────────────────────────────────────────────────

/// Verifies that `handle_agent_feed_event` returns `NoOp` when the channel is closed (None).
#[tokio::test]
async fn handle_agent_feed_event_returns_noop_on_none() {
    // Given: a conversation state, closed channel, and a fake logger
    let mut state = conversation_state();
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();

    // When: the agent feed event handler receives None
    let outcome = super::handle_agent_feed_event(&mut state, None, &logger);

    // Then: it returns NoOp because there is nothing to apply
    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "handle_agent_feed_event must return NoOp when the channel is closed"
    );
}

/// Verifies that `handle_agent_feed_event` returns `Redraw` when a TaskStarted event arrives.
#[tokio::test]
async fn handle_agent_feed_event_returns_redraw_on_task_started() {
    // Given: a conversation state and a TaskStarted event
    let mut state = conversation_state();
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let event = Some(FeedEntry {
        feed_id: FeedId::Agent("tui-events-tests".into()),
        output: AgentFeedOutput::TaskStarted {
            name: AgentName::new("test-agent"),
            model: None,
        },
    });

    // When: the agent feed event handler receives the event
    let outcome = super::handle_agent_feed_event(&mut state, event, &logger);

    // Then: it returns Redraw so the feed panel can update
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_agent_feed_event must return Redraw when a TaskStarted event is received"
    );
}

/// Verifies that `handle_agent_feed_event` pushes a system message when a TaskFailed event arrives.
#[tokio::test]
async fn handle_agent_feed_event_pushes_system_message_on_task_failed() {
    // Given: a conversation state and a TaskFailed event
    let mut state = conversation_state();
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let event = Some(FeedEntry {
        feed_id: FeedId::Agent("tui-events-tests".into()),
        output: AgentFeedOutput::TaskFailed {
            name: AgentName::new("failing-agent"),
            reason: OutputText::new("something went wrong"),
        },
    });

    // When: the agent feed event handler receives the TaskFailed event
    let outcome = super::handle_agent_feed_event(&mut state, event, &logger);

    // Then: the outcome is Redraw and a system message describing the failure was pushed
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_agent_feed_event must return Redraw on TaskFailed"
    );
    let has_failure_message =
        state.output.lines.iter().any(|l| {
            l.text.as_str().contains("failing-agent") && l.text.as_str().contains("failed")
        });
    assert!(
        has_failure_message,
        "handle_agent_feed_event must push a system message containing the agent name and 'failed' on TaskFailed"
    );
}

/// Verifies that `handle_agent_feed_event` returns `Redraw` when a Clear event arrives.
#[tokio::test]
async fn handle_agent_feed_event_returns_redraw_on_clear() {
    // Given: a conversation state and a Clear event
    let mut state = conversation_state();
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let event = Some(FeedEntry {
        feed_id: FeedId::Agent("tui-events-tests".into()),
        output: AgentFeedOutput::Clear,
    });

    // When: the agent feed event handler receives the Clear event
    let outcome = super::handle_agent_feed_event(&mut state, event, &logger);

    // Then: it returns Redraw
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_agent_feed_event must return Redraw on Clear"
    );
}

// ── handle_orchestrator_event ─────────────────────────────────────────────────

/// Verifies that `handle_orchestrator_event` returns `NoOp` on a lagged broadcast error.
#[tokio::test]
async fn handle_orchestrator_event_returns_noop_on_error() {
    // Given: a conversation state and a lagged broadcast error
    let mut state = conversation_state();
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let err = Err(tokio::sync::broadcast::error::RecvError::Lagged(1));

    // When: the orchestrator event handler receives a lagged error
    let outcome = super::handle_orchestrator_event(&mut state, err, &logger);

    // Then: it returns NoOp because lagged events are silently dropped
    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "handle_orchestrator_event must return NoOp on broadcast error"
    );
}

/// Verifies that `handle_orchestrator_event` pushes a system message and returns `Redraw` on a valid event.
#[tokio::test]
async fn handle_orchestrator_event_pushes_message_and_returns_redraw() {
    // Given: a conversation state and a valid orchestrator event
    let mut state = conversation_state();
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let event = Ok(DeterministicOrchestratorEvent::Completed);

    // When: the orchestrator event handler receives the event
    let outcome = super::handle_orchestrator_event(&mut state, event, &logger);

    // Then: it returns Redraw and pushed a system message with the formatted event text
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_orchestrator_event must return Redraw on a valid event"
    );
    let has_completed_message = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("[pipeline] completed"));
    assert!(
        has_completed_message,
        "handle_orchestrator_event must push a system message containing the formatted event text"
    );
}

/// Verifies that `handle_orchestrator_event` clears the active_task on `Halted`
/// without turning off the main thinking spinner.
#[tokio::test]
async fn handle_orchestrator_event_clears_thinking_on_halted() {
    // Given: a conversation state with active thinking and agent feed task
    let mut state = conversation_state();
    state.agent.thinking.is_active = true;
    state.interaction.panel.agent_feed.active_task = Some(TaskName::new("some-task"));
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let event = Ok(DeterministicOrchestratorEvent::Halted {
        step_id: WorkflowStepId::from("implement-behavior"),
    });

    // When: the orchestrator event handler receives a Halted event
    let outcome = super::handle_orchestrator_event(&mut state, event, &logger);

    // Then: thinking is cleared and active_task is None
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_orchestrator_event must return Redraw on Halted"
    );
    assert!(
        state.agent.thinking.is_active,
        "Halted event must not clear the main thinking spinner"
    );
    assert!(
        state.interaction.panel.agent_feed.active_task.is_none(),
        "Halted event must clear agent_feed.active_task"
    );
}

/// Verifies that `handle_orchestrator_event` clears the active_task on `Completed`
/// without turning off the main thinking spinner.
#[tokio::test]
async fn handle_orchestrator_event_clears_thinking_on_completed() {
    // Given: a conversation state with active thinking and agent feed task
    let mut state = conversation_state();
    state.agent.thinking.is_active = true;
    state.interaction.panel.agent_feed.active_task = Some(TaskName::new("running-task"));
    let (_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let event = Ok(DeterministicOrchestratorEvent::Completed);

    // When: the orchestrator event handler receives a Completed event
    let outcome = super::handle_orchestrator_event(&mut state, event, &logger);

    // Then: thinking is cleared and active_task is None
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_orchestrator_event must return Redraw on Completed"
    );
    assert!(
        state.agent.thinking.is_active,
        "Completed event must not clear the main thinking spinner"
    );
    assert!(
        state.interaction.panel.agent_feed.active_task.is_none(),
        "Completed event must clear agent_feed.active_task"
    );
}

// ── handle_agent_output_event (via TestRig) ───────────────────────────────────

/// Verifies that `handle_agent_output_event` returns `Redraw` and does not trigger compact
/// when a `CompactionComplete` output is received (compact is triggered by the guided plan actor,
/// not directly by the output event).
#[tokio::test]
async fn handle_agent_output_event_compaction_complete_does_not_increment_compact_calls() {
    // Given: a TestRig with a RecordingCompactProvider and a CompactionComplete output
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    let handles = rig.handles();
    let mut char_buf = OutputText::new("");

    let output: Result<AgentOutput, tokio::sync::broadcast::error::RecvError> =
        Ok(AgentOutput::CompactionComplete {
            text: OutputText::new("context compacted: 50000 → 12500 tokens"),
        });

    // When: the agent output event is applied
    let event_ctx = super::AgentOutputEventContext::new(&mut char_buf, &handles);
    let outcome = super::handle_agent_output_event(&mut state, output, event_ctx);

    // Then: the outcome is Redraw and the RecordingCompactProvider was not asked to compact
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "CompactionComplete output must produce Redraw"
    );
    assert_eq!(
        rig.provider.compact_call_count(),
        0,
        "CompactionComplete output must not directly call compact() on the provider"
    );
}

/// Verifies that `handle_agent_output_event` returns `Redraw` when a `Done` token is received.
#[tokio::test]
async fn handle_agent_output_event_done_returns_redraw() {
    // Given: a TestRig and a Done output
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    let handles = rig.handles();
    let mut char_buf = OutputText::new("");

    let output: Result<AgentOutput, tokio::sync::broadcast::error::RecvError> =
        Ok(AgentOutput::Done);

    // When: the agent output event is applied
    let event_ctx = super::AgentOutputEventContext::new(&mut char_buf, &handles);
    let outcome = super::handle_agent_output_event(&mut state, output, event_ctx);

    // Then: the outcome is Redraw
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "Done output must produce Redraw"
    );
}

/// Verifies that token output is buffered without forcing an immediate redraw.
#[tokio::test]
async fn handle_agent_output_event_token_buffers_without_immediate_redraw() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    let handles = rig.handles();
    let mut char_buf = OutputText::new("");

    let output: Result<AgentOutput, tokio::sync::broadcast::error::RecvError> =
        Ok(AgentOutput::Token(OutputText::new("streamed text")));

    let event_ctx = super::AgentOutputEventContext::new(&mut char_buf, &handles);
    let outcome = super::handle_agent_output_event(&mut state, output, event_ctx);

    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "Token output should wait for the tick-driven flush before redrawing"
    );
    assert!(
        !char_buf.is_empty(),
        "Token output must still be buffered for the next tick"
    );
}

// ── handle_guided_plan_update (via TestRig) ───────────────────────────────────

/// Verifies that `handle_guided_plan_update` returns `NoOp` on a broadcast error.
#[tokio::test]
async fn handle_guided_plan_update_returns_noop_on_error() {
    // Given: a TestRig, a conversation state, and a lagged broadcast error
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    let handles = rig.handles();
    let err = Err(tokio::sync::broadcast::error::RecvError::Lagged(1));

    // When: the guided plan update handler receives an error
    let outcome = super::handle_guided_plan_update(&mut state, err, &handles);

    // Then: it returns NoOp because errors are silently dropped
    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "handle_guided_plan_update must return NoOp on broadcast error"
    );
}

/// Verifies that `handle_guided_plan_update` returns `Redraw` when a valid event arrives.
#[tokio::test]
async fn handle_guided_plan_update_returns_redraw_on_valid_event() {
    use crate::domain::guided_plan::GuidedPlanEvent;

    // Given: a TestRig, a conversation state, and a valid PlanComplete event
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    let handles = rig.handles();
    let event = Ok(GuidedPlanEvent::PlanComplete);

    // When: the guided plan update handler receives the event
    let outcome = super::handle_guided_plan_update(&mut state, event, &handles);

    // Then: it returns Redraw so the guided plan panel can update
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_guided_plan_update must return Redraw when a valid GuidedPlanEvent is received"
    );
}

/// BH-TKN-039: TUI actor dispatches a usage snapshot to app state on snapshot tick.
///
/// Verifies that `handle_snapshot_tick` reads the token tracker's current snapshot
/// and applies it to `state.status.token_totals`, returning `Redraw` so the status
/// bar is refreshed.
#[tokio::test]
async fn test_tui_actor_dispatches_usage_snapshot_on_tick() {
    use crate::actors::token_tracker;
    use crate::domain::newtypes::NumericNewtype;
    use crate::domain::string_newtypes::{OutputText, StringNewtype};
    use crate::domain::types::{LlmTokenCounts, LlmUsage};
    use crate::domain::Temperature;
    use crate::domain::TokenCount;

    // Given: a token tracker with existing usage totals.
    let dir = tempfile::tempdir().expect("tempdir for token tracker");
    let _settings_path = dir.path().join("settings.json");
    let (_join, tracker_handle) = token_tracker::spawn();
    tracker_handle.record_usage(LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(100),
            tokens_out: TokenCount::ZERO,
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    });

    let mut state = conversation_state();

    // When: the snapshot tick handler is called
    let outcome = super::handle_snapshot_tick(&mut state, &tracker_handle).await;

    // Then: it returns Redraw and the state reflects the seeded totals
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "handle_snapshot_tick must return Redraw to refresh the status bar"
    );
    assert_eq!(
        state.status.token_totals.tokens_in,
        TokenCount::new(100),
        "handle_snapshot_tick must apply the snapshot to state.status.token_totals"
    );
}

/// Verifies `/new-session` causes subsequent snapshot ticks to show only session-local totals.
#[tokio::test]
async fn snapshot_tick_uses_new_session_baseline_after_reset() {
    use crate::actors::token_tracker;
    use crate::domain::newtypes::NumericNewtype;
    use crate::domain::string_newtypes::{OutputText, StringNewtype};
    use crate::domain::types::{LlmTokenCounts, LlmUsage};
    use crate::domain::Temperature;
    use crate::domain::TokenCount;

    let dir = tempfile::tempdir().expect("tempdir for token tracker");
    let _settings_path = dir.path().join("settings.json");
    let (_join, tracker_handle) = token_tracker::spawn();

    let usage = |tokens_in| LlmUsage {
        model: OutputText::new("m"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(tokens_in),
            tokens_out: TokenCount::ZERO,
            tokens_cached: TokenCount::ZERO,
            cache_write_tokens: TokenCount::ZERO,
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.0),
    };

    tracker_handle.record_usage(usage(100));
    let mut state = conversation_state();
    let _ = super::handle_snapshot_tick(&mut state, &tracker_handle).await;
    assert_eq!(state.status.token_totals.tokens_in, TokenCount::new(100));

    state.reset_for_new_session();
    let _ = super::handle_snapshot_tick(&mut state, &tracker_handle).await;
    assert_eq!(
        state.status.token_totals.tokens_in,
        TokenCount::ZERO,
        "first tick after new session must capture baseline and display zero"
    );

    tracker_handle.record_usage(usage(7));
    let _ = super::handle_snapshot_tick(&mut state, &tracker_handle).await;
    assert_eq!(
        state.status.token_totals.tokens_in,
        TokenCount::new(7),
        "snapshot after reset must show only post-reset usage"
    );
}

// ── handle_picker_agent_output ────────────────────────────────────────────────

/// Verifies that `handle_picker_agent_output` replaces stale models on each
/// `ModelsAvailable` refresh.
#[test]
fn handle_picker_agent_output_replaces_model_list_on_successive_events() {
    use crate::domain::string_newtypes::{ModelId, ModelLabel};
    use crate::domain::tui_state::{AppScreen, PickerState};
    use crate::domain::types::ModelOption;

    // Given: a state in SessionSelector screen with an empty model list
    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![],
            selected: crate::domain::newtypes::Count::of(0),
        }),
    );

    let first_batch = vec![ModelOption::builder()
        .id(ModelId::new("openrouter-sonnet"))
        .display_name(ModelLabel::new("claude-sonnet-4-5 (openrouter)"))
        .build()];
    let second_batch = vec![ModelOption::builder()
        .id(ModelId::new("copilot-gpt-4o"))
        .display_name(ModelLabel::new("gpt-4o (copilot)"))
        .build()];

    // When: two successive ModelsAvailable events arrive
    let _ = super::handle_picker_agent_output(
        &mut state,
        Ok(AgentOutput::ModelsAvailable(first_batch)),
    );
    let _ = super::handle_picker_agent_output(
        &mut state,
        Ok(AgentOutput::ModelsAvailable(second_batch)),
    );

    // Then: only the newest batch remains (replace, not extend)
    assert_eq!(
        state.prompt.models.available.len(),
        1,
        "handle_picker_agent_output must replace stale models with the newest batch"
    );
    assert_eq!(
        state.prompt.models.available[0].id.as_str(),
        "copilot-gpt-4o",
        "only the newest ModelsAvailable batch should remain"
    );
}

#[test]
fn handle_picker_agent_output_ignores_legacy_models_for_non_auto_endpoint() {
    use crate::domain::string_newtypes::{ModelId, ModelLabel};
    use crate::domain::tui_state::{AppScreen, EndpointModelCatalog, PickerState};
    use crate::domain::types::ModelOption;

    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![],
            selected: crate::domain::newtypes::Count::of(0),
        }),
    );
    state.prompt.models.endpoint_catalog = vec![EndpointModelCatalog::builder()
        .endpoint_name(EndpointName::new("ep"))
        .models(vec![])
        .default_display("yaml-default".into())
        .supports_auto(false)
        .build()];
    state.prompt.models.available = vec![ModelOption::builder()
        .id(ModelId::new("yaml/model"))
        .display_name(ModelLabel::new("YAML Model"))
        .build()];

    let _ = super::handle_picker_agent_output(
        &mut state,
        Ok(AgentOutput::ModelsAvailable(vec![ModelOption::builder()
            .id(ModelId::new("legacy/endpoint-name"))
            .display_name(ModelLabel::new("Legacy Endpoint"))
            .build()])),
    );

    assert_eq!(
        state.prompt.models.available[0].id.as_str(),
        "yaml/model",
        "picker-mode ModelsAvailable must not override YAML-backed non-auto endpoint models"
    );
}
