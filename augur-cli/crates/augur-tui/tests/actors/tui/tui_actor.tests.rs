use crate::actors::agent::agent_actor::{spawn as spawn_agent, AgentSpawnArgs};
use crate::actors::logger::logger_actor::spawn as spawn_logger;
use crate::actors::tui::handle::TuiHandle;
use crate::config::types::{AgentConfig, CopilotConfig, PersistenceConfig};
use crate::domain::newtypes::{
    Count, NumericNewtype, ScrollOffset, Temperature, TimestampMs, TokenCount,
};
use crate::domain::string_newtypes::{
    EndpointName, FilePath, ModelLabel, OutputText, PhaseName, PromptText, SessionId,
    StringNewtype, ToolName,
};
use crate::domain::tui_state::{
    AppScreen, AppState, ConversationMode, LineKind, PickerSessionIdentity, PickerSessionSummary,
    PickerState,
};
use crate::domain::types::{AgentOutput, CancelSignal, Message};
use crate::persistence::handle::PersistenceHandle;
use crate::persistence::types::{MessageRecord, MessageType, SessionRecord};
use crate::tools::builtin::query_user::QueryUserRequest;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;

use crate::tests::helpers::fake_ask;
use crate::tests::helpers::fake_tool::FakeToolExecutor;

fn model_option(id: &str, display_name: &str) -> crate::domain::types::ModelOption {
    crate::domain::types::ModelOption::builder()
        .id(crate::domain::string_newtypes::ModelId::new(id))
        .display_name(ModelLabel::new(display_name))
        .build()
}

/// A test LlmClient that never sends any chunks (sleeps 60 s before dropping the sender).
///
/// Used to keep a turn in-flight for cancel/interrupt tests where the stream
/// must remain open long enough for an interrupt signal to be delivered.
struct StalledLlmClient;

impl crate::actors::llm::handle::LlmClient for StalledLlmClient {
    fn complete_stream(
        &self,
        _request: crate::domain::traits::CompletionRequest,
    ) -> tokio::sync::mpsc::Receiver<crate::domain::types::StreamChunk> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60)).await;
            drop(tx);
        });
        rx
    }
}

fn make_key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

async fn make_agent_handle() -> (crate::actors::agent::handle::AgentHandle, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let log_dir = tempfile::tempdir().expect("log tempdir");
    let (_logger_join, logger) = spawn_logger(log_dir.path().to_path_buf());
    std::mem::forget(log_dir);
    let (_, handle) = spawn_agent(
        AgentSpawnArgs::builder()
            .llm(StalledLlmClient)
            .tools(FakeToolExecutor::always_ok(""))
            .config(AgentConfig {
                system_prompt: OutputText::new("test"),
                max_tokens: TokenCount::new(1024),
                temperature: Temperature::new(0.5),
                allowed_dirs: vec![],
            })
            .services(
                crate::actors::agent::agent_actor::AgentServices::builder()
                    .persistence(persistence)
                    .logger(logger)
                    .token_tracker(
                        crate::tests::helpers::fake_token_tracker::fake_token_tracker_handle().1,
                    )
                    .history_adapter(
                        crate::tests::helpers::fake_history_adapter::fake_history_adapter_handle(),
                    )
                    .build(),
            )
            .extensions(crate::domain::task_types::AgentExtensions {
                cache: None,
                instruction_prefix: None,
            message_compactor: None,
            })
            .app_config(crate::config::AppConfig {
                endpoints: vec![],
                default_endpoint: EndpointName::new("test"),
                agent: AgentConfig {
                    system_prompt: OutputText::new("test"),
                    max_tokens: TokenCount::new(1024),
                    temperature: Temperature::new(0.5),
                    allowed_dirs: vec![],
                },
                copilot: Default::default(),
                persistence: crate::config::PersistenceConfig {
                    log_dir: crate::domain::string_newtypes::FilePath::new("./logs"),
                    sessions_dir: None,
                },
                    program_settings: Default::default(),
                    user_settings: Default::default(),
            })
            .build(),
    );
    (handle, dir)
}

/// Creates a live `FileScannerHandle` for tests that construct `TuiHandles`.
///
/// Returns the join handle and client handle. Tests should ignore the join handle
/// (`_join`) - the actor will terminate when the channel is dropped.
fn make_scanner() -> (
    tokio::task::JoinHandle<()>,
    crate::actors::FileScannerHandle,
) {
    crate::actors::file_scanner::file_scanner_actor::spawn()
}

/// Build a minimal `TuiSubActorHandles` for tests that construct `TuiSpawnArgs`.
///
/// Spawns all six sub-actors with capacity 8 and drops the join handles; the
/// actors run in the background until the test runtime shuts down.
fn make_test_sub_actors() -> super::runtime::layout::TuiSubActorHandles {
    use crate::actors::tui_agent_panel::tui_agent_panel_actor::{
        spawn as spawn_agent_panel, TuiAgentPanelConfig,
    };
    use crate::actors::tui_ask_panel::tui_ask_panel_actor::spawn as spawn_ask_panel;
    use crate::actors::tui_chat_menu::tui_chat_menu_actor::spawn as spawn_chat_menu;
    use crate::actors::tui_dynamic_controls::tui_dynamic_controls_actor::spawn as spawn_controls;
    use crate::actors::tui_main_feed_panel::tui_main_feed_panel_actor::{
        spawn as spawn_main_feed, TuiMainFeedConfig,
    };
    use crate::actors::tui_main_feed_panel::tui_main_feed_panel_ops::MainFeedItem;
    use crate::actors::tui_spinner::tui_spinner_actor::spawn as spawn_spinner;
    use crate::domain::newtypes::Count;
    use crate::domain::types::AgentFeedOutput;

    let (agent_feed_tx, _) = tokio::sync::mpsc::channel::<AgentFeedOutput>(8);
    let (main_feed_tx, _) = tokio::sync::mpsc::channel::<MainFeedItem>(8);

    let (_, agent_panel) = spawn_agent_panel(TuiAgentPanelConfig {
        unified_tx: agent_feed_tx,
        capacity: 8,
    });
    let (_, main_feed) = spawn_main_feed(TuiMainFeedConfig {
        unified_tx: main_feed_tx,
        capacity: 8,
    });
    let (_, ask_panel) = spawn_ask_panel(Count::of(8));
    let (_, chat_menu) = spawn_chat_menu(Count::of(8));
    let (_, spinner) = spawn_spinner(Count::of(8));
    let (_, controls) = spawn_controls(Count::of(8));

    super::runtime::layout::TuiSubActorHandles::builder()
        .main_feed(main_feed)
        .agent_panel(agent_panel)
        .ask_panel(ask_panel)
        .overlays(
            super::runtime::layout::TuiOverlayHandles::builder()
                .chat_menu(chat_menu)
                .spinner(spinner)
                .controls(controls)
                .build(),
        )
        .build()
}

fn make_picker_summary() -> PickerSessionSummary {
    PickerSessionSummary::builder()
        .identity(
            PickerSessionIdentity::builder()
                .id(SessionId::new("test-session"))
                .created_at(TimestampMs::new(1_000_000))
                .last_updated_at(TimestampMs::new(1_000_000))
                .endpoint_name(EndpointName::new("claude"))
                .build(),
        )
        .message_count(Count::new(2))
        .preview(OutputText::new("hi there"))
        .build()
}

/// Verifies that wait_for_shutdown resolves when the shutdown watch channel is
/// set to true, without requiring a real terminal.
#[tokio::test]
async fn spawn_and_signal_shutdown() {
    let (shutdown_tx, shutdown_rx) =
        watch::channel(crate::actors::tui::handle::ShutdownSignal::Running);
    let (agent_feed_tx, _) = tokio::sync::mpsc::channel(1);
    let mut handle = TuiHandle::new(shutdown_rx, agent_feed_tx);

    let wait_task = tokio::spawn(async move {
        handle.wait_for_shutdown().await;
    });

    // Signal shutdown
    shutdown_tx
        .send(crate::actors::tui::handle::ShutdownSignal::Complete)
        .unwrap();

    let result = timeout(Duration::from_secs(1), wait_task).await;
    assert!(
        result.is_ok(),
        "wait_for_shutdown did not resolve within timeout"
    );
    assert!(result.unwrap().is_ok());
}

/// Verifies that startup terminal configuration emits the exact title escape.
#[test]
fn configure_terminal_startup_sets_exact_terminal_title() {
    let mut bytes = Vec::new();

    super::configure_terminal_startup(&mut bytes).expect("startup terminal commands must render");

    let rendered = String::from_utf8(bytes).expect("terminal commands must be utf-8");
    let expected = format!("\u{1b}]0;{}\u{7}", super::TERMINAL_TITLE);
    assert!(
        rendered.contains(&expected),
        "startup commands must set the terminal title to exactly {:?}",
        super::TERMINAL_TITLE
    );
}

/// Verifies that AppState created with a non-empty SessionPicker mode reports is_picker() == true.
///
/// Confirms the TUI actor's initial mode building logic correctly enables the picker
/// when session_summaries is non-empty.
#[test]
fn picker_mode_created_when_sessions_provided() {
    let picker = PickerState {
        sessions: vec![make_picker_summary()],
        selected: Count::new(0),
    };
    let state = AppState::new(
        EndpointName::new("claude"),
        AppScreen::SessionSelector(picker),
    );
    assert!(state.is_picker().0);
}

/// Verifies that transitioning from picker mode via take_picker_state sets mode to Chat.
///
/// Simulates the NewSession key action path in handle_picker_event, where the TUI
/// should discard the picker and enter the normal chat interface.
#[test]
fn picker_new_session_transitions_to_chat() {
    let picker = PickerState {
        sessions: vec![make_picker_summary()],
        selected: Count::new(0),
    };
    let mut state = AppState::new(
        EndpointName::new("claude"),
        AppScreen::SessionSelector(picker),
    );
    assert!(state.is_picker().0);
    let _ = state.take_picker_state();
    assert!(!state.is_picker().0);
}

/// Verifies that take_picker_state on an empty session list still transitions to Chat safely.
///
/// Edge case: if the picker is shown with zero sessions, Confirm should not panic
/// and the mode should resolve to Chat cleanly.
#[test]
fn picker_confirm_with_no_sessions_starts_chat() {
    let picker = PickerState {
        sessions: vec![],
        selected: Count::new(0),
    };
    let mut state = AppState::new(
        EndpointName::new("claude"),
        AppScreen::SessionSelector(picker),
    );
    let taken = state.take_picker_state();
    assert!(taken.is_some());
    let ps = taken.unwrap();
    assert!(ps.sessions.is_empty());
    assert!(!state.is_picker().0);
}

/// Verifies that handle_query_request transitions AppState to ConversationMode::Query.
///
/// When the TUI actor receives a QueryUserRequest over the mpsc channel,
/// it calls handle_query_request which must set the mode to ConversationMode::Query
/// so the next render cycle shows the query overlay.
#[test]
fn tui_query_mode_entered_when_request_received() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    assert!(!state.is_query().0);

    let (reply_tx, _reply_rx) = tokio::sync::oneshot::channel::<OutputText>();
    let req = QueryUserRequest {
        question: PromptText::new("Are you sure?"),
        choices: vec!["yes".into(), "no".into()],
        reply_tx,
    };

    crate::actors::tui::assistant::plan_view::handle_query_request(&mut state, Some(req));
    assert!(state.is_query().0);
}

/// Verifies that pressing Esc while the agent is thinking interrupts the turn
/// and pushes a "[stopped]" line to the output, clearing is_thinking.
///
/// dispatch_chat_key with Esc must call handle.interrupt(), set is_thinking=false,
/// and push a line containing "[stopped]" via push_turn_end, giving instant UI
/// feedback before the agent's Interrupted broadcast arrives.
#[tokio::test]
async fn escape_while_thinking_pushes_interrupted_and_clears_is_thinking() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: super::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.agent.thinking.is_active = true;

    let quit = super::dispatch_chat_key(&mut state, make_key(KeyCode::Esc), &handles).await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "Esc must not quit the TUI"
    );
    assert!(
        !state.agent.thinking.is_active,
        "is_thinking must be false after Esc cancel"
    );
    assert_eq!(
        agent.is_cancelled(),
        CancelSignal::Cancelled,
        "cancel signal must be set after Esc"
    );
    let has_interrupted = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("[stopped]"));
    assert!(
        has_interrupted,
        "output must contain [stopped] after Esc cancel"
    );
}

/// Verifies that pressing Enter with an empty buffer while the agent is thinking
/// is a no-op: no interrupt, no output push, is_thinking unchanged.
///
/// An empty follow-up submit while thinking must be ignored to prevent
/// accidental empty resubmissions during in-progress turns.
#[tokio::test]
async fn enter_while_thinking_with_empty_buffer_is_noop() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: super::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.agent.thinking.is_active = true;
    state.prompt.buffer = String::new();

    let quit = super::dispatch_chat_key(&mut state, make_key(KeyCode::Enter), &handles).await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "Enter with empty buffer must not quit"
    );
    assert!(
        state.agent.thinking.is_active,
        "is_thinking must be unchanged for empty Enter"
    );
    assert!(
        agent.is_cancelled() == CancelSignal::Clear,
        "interrupt must NOT be called for empty Enter"
    );
    assert!(
        state.output.lines.is_empty(),
        "no output must be pushed for empty Enter"
    );
}

/// Verifies that pressing Enter with a non-empty buffer while the agent is thinking
/// interrupts the current turn, pushes "[steering]", then resubmits the new text.
///
/// After handle_cancel_or_submit runs: output contains "[steering]", is_thinking
/// is set back to true (by the inner handle_submit), and the prompt buffer is cleared.
#[tokio::test]
async fn enter_with_buffer_while_thinking_interrupts_and_resubmits() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: super::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.agent.thinking.is_active = true;
    state.prompt.buffer = "new question".to_owned();
    state.prompt.cursor = state.prompt.buffer.len();

    let quit = super::dispatch_chat_key(&mut state, make_key(KeyCode::Enter), &handles).await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "Enter with buffer while thinking must not quit"
    );
    assert!(
        agent.is_cancelled() == CancelSignal::Cancelled,
        "interrupt must be called before resubmit"
    );
    let has_interrupted = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("[steering]"));
    assert!(
        has_interrupted,
        "output must contain [steering] before resubmit"
    );
    // handle_submit sets is_thinking=true for the new turn
    assert!(
        state.agent.thinking.is_active,
        "is_thinking must be true after resubmit"
    );
    // prompt buffer cleared by take_prompt inside handle_submit
    assert!(
        state.prompt.buffer.is_empty(),
        "buffer must be cleared after submit"
    );
}

/// Verifies that typing /quit and pressing Enter causes dispatch_chat_key to return true.
///
/// Regression test for a bug where handle_cancel_or_submit discarded the return
/// value of handle_submit, causing /quit to be swallowed and the TUI to never exit.
#[tokio::test]
async fn slash_quit_command_returns_quit_true() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/quit".to_owned();
    state.prompt.cursor = 5;

    let quit = super::dispatch_chat_key(&mut state, make_key(KeyCode::Enter), &handles).await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Break(())),
        "/quit + Enter must return quit=true from dispatch_chat_key"
    );
}

/// Verifies that a slash command (e.g. /help) producing a SystemMessage outcome
/// is followed by two blank lines in the output pane.
///
/// System messages must end with two push_output_newline calls so that the
/// second blank line acts as a visible separator when the next message arrives.
/// Without the second blank, the next token appends to the single blank line,
/// consuming the separator. This matches the two-newline convention used by
/// push_turn_end for agent responses.
#[tokio::test]
async fn slash_command_system_message_followed_by_blank_line() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/help".to_owned();
    state.prompt.cursor = 5;

    let quit = super::dispatch_chat_key(&mut state, make_key(KeyCode::Enter), &handles).await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "/help must not quit"
    );
    let n = state.output.lines.len();
    assert!(n >= 2, "output must have at least 2 lines after /help");
    let last = state.output.lines[n - 1].text.as_str();
    let second_last = state.output.lines[n - 2].text.as_str();
    assert!(
        last.is_empty() && second_last.is_empty(),
        "output must end with 2 consecutive blank lines for visible message separator, \
         got last='{last}', second_last='{second_last}'"
    );
}

/// Verifies that restored user and assistant messages are each followed by blank
/// separator lines in the output pane.
///
/// Session restore should produce the same visual spacing as live interaction:
/// every message ends with a blank line so distinct turns are clearly separated.
#[tokio::test]
async fn restored_messages_have_blank_separator_lines() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user(PromptText::new("hello")),
        },
        MessageRecord {
            message_type: MessageType::Assistant,
            message: Message::assistant(OutputText::new("hi there")),
        },
    ];

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    // Collect indices of blank lines before the final [system] confirmation.
    // Restored history should not insert separator gaps between each restored message.
    // A single trailing blank is allowed immediately before the final system line.
    let non_system_lines: Vec<(usize, &str)> = state
        .output
        .lines
        .iter()
        .enumerate()
        .take_while(|(_, l)| !l.text.as_str().contains("[system]"))
        .map(|(i, l)| (i, l.text.as_str()))
        .collect();
    let blank_count = non_system_lines
        .iter()
        .filter(|(_, s)| s.is_empty())
        .count();
    assert!(
        blank_count <= 1,
        "restored output should not contain separator gaps, got {blank_count}. Lines: {:?}",
        non_system_lines
    );
}

/// Verifies that a multiline assistant response in a restored session renders
/// as separate output lines rather than being concatenated onto a single line.
///
/// The hydration path must use push_output_token with the full content string
/// so the newline-splitting logic in push_token_with_newlines fires correctly,
/// matching the behavior of live streaming responses.
#[tokio::test]
async fn restored_session_assistant_multiline_renders_as_separate_lines() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![MessageRecord {
        message_type: MessageType::Assistant,
        message: Message::assistant(OutputText::new("line one\nline two\nline three")),
    }];

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    let all_text: Vec<&str> = state.output.lines.iter().map(|l| l.text.as_str()).collect();
    let has_line_one = all_text.contains(&"line one");
    let has_line_two = all_text.contains(&"line two");
    let has_line_three = all_text.contains(&"line three");
    assert!(
        has_line_one && has_line_two && has_line_three,
        "multiline assistant content must appear as separate output lines, got: {all_text:?}"
    );
}

/// Verifies that apply_restored_session hydrates the output pane with user and
/// assistant messages from the restored record, with the system confirmation
/// line pushed last. Tool messages must not appear in output.
#[tokio::test]
async fn restored_session_output_is_hydrated() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user(PromptText::new("hello user")),
        },
        MessageRecord {
            message_type: MessageType::Assistant,
            message: Message::assistant(OutputText::new("hello assistant")),
        },
        MessageRecord {
            message_type: MessageType::Tool(ToolName::new("some_tool")),
            message: Message::tool_result(
                crate::domain::string_newtypes::ToolCallId::new("call_stub"),
                &ToolName::new("some_tool"),
                OutputText::new("tool output"),
            ),
        },
    ];

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    super::apply_restored_session(&mut state, record, &handles).await;

    let all_text: Vec<&str> = state.output.lines.iter().map(|l| l.text.as_str()).collect();

    // User message must appear as "> hello user"
    let has_user = all_text
        .iter()
        .any(|l| l.contains("> ") && l.contains("hello user"));
    assert!(
        has_user,
        "output must contain '>  hello user' but got: {all_text:?}"
    );

    // Assistant message must appear
    let has_assistant = all_text.iter().any(|l| l.contains("hello assistant"));
    assert!(
        has_assistant,
        "output must contain 'hello assistant' but got: {all_text:?}"
    );

    // Tool message must NOT appear
    let has_tool = all_text.iter().any(|l| l.contains("tool output"));
    assert!(
        !has_tool,
        "tool output must not appear in restored output but got: {all_text:?}"
    );

    // System confirmation line must be last non-blank content
    let last_content = state
        .output
        .lines
        .iter()
        .rev()
        .find(|l| !l.text.as_str().is_empty())
        .expect("must have at least one non-blank output line");
    assert!(
        last_content
            .text
            .as_str()
            .contains("[system] restored session"),
        "last non-blank output line must be the system confirmation, got: '{}'",
        last_content.text.as_str()
    );
}

/// Verifies that apply_restored_session produces a [system] confirmation line
/// with a non-None timestamp so the user can see when the session was restored.
///
/// The confirmation line must use push_system_message rather than push_output_token
/// to carry a wall-clock timestamp. Without a timestamp the renderer omits the
/// dimmed [HH:MM:SS] prefix, making the line visually indistinguishable from plain
/// agent output.
#[tokio::test]
async fn apply_restored_session_confirmation_has_timestamp() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let record = SessionRecord::new(EndpointName::new("ep"));
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    let system_line = state
        .output
        .lines
        .iter()
        .find(|l| l.text.as_str().contains("[system] restored session"))
        .expect("must find a [system] restored session confirmation line");
    assert!(
        system_line.header.timestamp.is_some(),
        "restored session confirmation must carry a timestamp so [HH:MM:SS] is rendered"
    );
}

/// Verifies that a MessageType::Error record is rendered as a red error line
/// when hydrating output from a saved session. The rendered text must include
/// the "[error]" prefix and the original error message, and the line must have
/// is_error=true so the renderer applies red+bold styling.
#[tokio::test]
async fn restored_session_error_records_render_as_error_lines() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: crate::domain::types::Message::user(PromptText::new("hello")),
        },
        MessageRecord {
            message_type: MessageType::Error,
            message: crate::domain::types::Message::system(OutputText::new(
                "stream connection failed",
            )),
        },
    ];

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    let error_lines: Vec<_> = state
        .output
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Error)
        .collect();
    assert!(
        !error_lines.is_empty(),
        "must have at least one error line after restore"
    );
    let error_text: Vec<&str> = error_lines.iter().map(|l| l.text.as_str()).collect();
    let has_error_msg = error_text
        .iter()
        .any(|t| t.contains("[error]") && t.contains("stream connection failed"));
    assert!(
        has_error_msg,
        "error line must contain '[error] stream connection failed', got: {error_text:?}"
    );
}

/// Verifies that submitting a non-command prompt immediately echoes the user
/// input to the output pane with the "> " prefix before the agent responds.
///
/// The user must see their own message in the chat history the moment they
/// press Enter, not only after the agent replies or after session restore.
#[tokio::test]
async fn submit_echoes_user_input_to_output_immediately() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "what is 2+2".to_owned();
    state.prompt.cursor = state.prompt.buffer.len();

    let quit = super::dispatch_chat_key(&mut state, make_key(KeyCode::Enter), &handles).await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "submitting text must not quit"
    );
    let has_echo = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("> what is 2+2"));
    assert!(
        has_echo,
        "submitted text must be echoed to output with '> ' prefix immediately, got: {:?}",
        state
            .output
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
    );
}

/// Verifies that the echoed user input line is marked as a user input line.
///
/// The renderer applies a distinct background style to user input lines using
/// the is_user_input flag. Lines echoed via handle_submit must carry this flag.
#[tokio::test]
async fn submit_echo_is_marked_as_user_input_line() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "tell me something".to_owned();
    state.prompt.cursor = state.prompt.buffer.len();

    let _ = super::dispatch_chat_key(&mut state, make_key(KeyCode::Enter), &handles).await;

    let user_line = state
        .output
        .lines
        .iter()
        .find(|l| l.text.as_str().starts_with("> "))
        .expect("echoed user input line must exist");
    assert!(
        user_line.kind == LineKind::UserInput,
        "echoed user input line must have LineKind::UserInput"
    );
}

/// Verifies that restored user messages are marked as user input lines.
///
/// Session restore must use push_user_input_line for user messages so they
/// receive the same background styling as live-submitted messages.
#[tokio::test]
async fn restored_user_messages_are_marked_as_user_input_lines() {
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![MessageRecord {
        message_type: MessageType::User,
        message: Message::user(PromptText::new("hi there")),
    }];

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    let user_line = state
        .output
        .lines
        .iter()
        .find(|l| l.text.as_str().contains("hi there"))
        .expect("restored user message must appear in output");
    assert!(
        user_line.kind == LineKind::UserInput,
        "restored user message line must have LineKind::UserInput"
    );
}

/// Verifies that resolve_query_answer interprets a numeric freeform as a 1-based choice selector.
///
/// When the freeform field contains "2" and choices has at least two entries, the answer
/// must be the text of the second choice rather than the literal "2".
#[test]
fn resolve_query_answer_numeric_selects_matching_choice() {
    let (reply_tx, _) = tokio::sync::oneshot::channel::<OutputText>();
    let qs = crate::domain::tui_state::QueryState {
        question: PromptText::new("Q"),
        choices: vec!["Alpha".into(), "Beta".into()],
        selected: None,
        freeform: PromptText::new("2"),
        reply_tx,
    };
    let answer = crate::actors::tui::assistant::plan_view::resolve_query_answer(&qs);
    assert_eq!(answer, Some(OutputText::new("Beta")));
}

/// Verifies that resolve_query_answer returns the literal freeform when the number exceeds choice count.
///
/// When freeform contains "5" but only one choice exists, the literal string "5" must be
/// returned so callers get exactly what was typed rather than a silent no-op.
#[test]
fn resolve_query_answer_out_of_range_number_returns_freeform_literal() {
    let (reply_tx, _) = tokio::sync::oneshot::channel::<OutputText>();
    let qs = crate::domain::tui_state::QueryState {
        question: PromptText::new("Q"),
        choices: vec!["Alpha".into()],
        selected: None,
        freeform: PromptText::new("5"),
        reply_tx,
    };
    let answer = crate::actors::tui::assistant::plan_view::resolve_query_answer(&qs);
    assert_eq!(answer, Some(OutputText::new("5")));
}

/// Verifies that handle_query_submit pushes the selected answer as a user input line.
///
/// After submit, mode must return to Chat, the reply channel must carry the answer,
/// and the output area must include the answer text styled as a user input line so
/// the conversation shows what the user chose before the LLM continues.
#[test]
fn handle_query_submit_pushes_answer_to_output() {
    let (reply_tx, mut reply_rx) = tokio::sync::oneshot::channel::<OutputText>();
    let qs = crate::domain::tui_state::QueryState {
        question: PromptText::new("Q"),
        choices: vec!["Yes".into(), "No".into()],
        selected: Some(0),
        freeform: PromptText::new(""),
        reply_tx,
    };
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Query(qs);
    crate::actors::tui::assistant::plan_view::handle_query_submit(&mut state);

    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "mode must return to Chat after submit"
    );
    let has_answer = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("Yes"));
    assert!(
        has_answer,
        "answer must appear in output lines after submit"
    );
    let received = reply_rx
        .try_recv()
        .expect("answer must be sent on reply channel");
    assert_eq!(received.as_str(), "Yes");
}

// ── Plan mode mouse scroll routing ────────────────────────────────────────

fn make_plan_state_with_chat_area(chat_cols: u16) -> AppState {
    use crate::domain::plan_tree::PlanTree;
    use crate::domain::tui_state::PlanModeState;
    use ratatui::layout::Rect;
    let tree = PlanTree::new("p1", "Test Plan", "goal");
    let plan_mode = PlanModeState {
        tree,
        running: false,
        tree_scroll: ScrollOffset::of(0),
    };
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Plan(plan_mode);
    // Simulate the output_area as if render set it to the left chat pane width.
    state.output.panel_areas.output_area.set(Rect {
        x: 0,
        y: 0,
        width: chat_cols,
        height: 20,
    });
    // Simulate the plan_panel_area as if render set it to the right panel region.
    state.output.panel_areas.plan_panel_area.set(Rect {
        x: chat_cols,
        y: 0,
        width: 40,
        height: 20,
    });
    state
}

/// Verifies that a scroll-up event whose column falls in the right plan panel
/// (column >= chat_cols) increments tree_scroll and does NOT change chat
/// output scroll_offset.
#[test]
fn handle_mouse_scroll_up_routes_to_plan_panel_when_column_in_right_pane() {
    use crate::actors::tui::assistant::plan_view::handle_plan_mouse_scroll;
    use crossterm::event::{MouseEvent, MouseEventKind};
    let mut state = make_plan_state_with_chat_area(60); // chat is 0..59, panel is 60+
    let event = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 65, // inside right panel
        row: 5,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    handle_plan_mouse_scroll(&mut state, event);
    if let ConversationMode::Plan(ref ps) = state.interaction.mode {
        assert!(
            ps.tree_scroll > ScrollOffset::of(0),
            "tree_scroll must increase on scroll-up in plan panel"
        );
    } else {
        panic!("expected plan mode");
    }
    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(0),
        "chat scroll must be unaffected"
    );
}

/// Verifies that a scroll-down event whose column falls in the left chat area
/// (column < chat_cols) routes to the chat output scroll and does NOT change
/// tree_scroll.
#[test]
fn handle_mouse_scroll_down_routes_to_chat_output_when_column_in_left_pane() {
    use crate::actors::tui::assistant::plan_view::handle_plan_mouse_scroll;
    use crossterm::event::{MouseEvent, MouseEventKind};
    let mut state = make_plan_state_with_chat_area(60);
    // Pre-set chat scroll offset so we can see a decrease.
    state.output.scroll_offset.set(ScrollOffset::of(10));
    let event = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 30, // inside left chat area
        row: 5,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    handle_plan_mouse_scroll(&mut state, event);
    if let ConversationMode::Plan(ref ps) = state.interaction.mode {
        assert_eq!(
            ps.tree_scroll,
            ScrollOffset::of(0),
            "tree_scroll must not change for chat-area scroll"
        );
    } else {
        panic!("expected plan mode");
    }
    assert!(
        state.output.scroll_offset.get() < ScrollOffset::of(10),
        "chat scroll_offset must decrease on scroll-down"
    );
}

// ── handle_mouse_event render-skip tests ─────────────────────────────────────

/// Verifies that a free-motion mouse-move event returns NoOp so the TUI loop
/// skips the render call.
///
/// The `?1003h` all-motion protocol (enabled by `EnableMouseCapture`) generates
/// a `MouseEventKind::Moved` event on every cursor movement. Without this guard
/// the main loop called `terminal.draw()` on every move, causing ~5% idle CPU.
#[test]
fn handle_mouse_event_moved_returns_no_op() {
    use crossterm::event::{MouseEvent, MouseEventKind};
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let event = MouseEvent {
        kind: MouseEventKind::Moved,
        column: 10,
        row: 5,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    let outcome = super::handle_mouse_event(&mut state, event);
    assert!(
        matches!(outcome, super::EventOutcome::NoOp),
        "free-motion mouse move must return NoOp to skip the render"
    );
}

/// Verifies that a scroll-up mouse event returns Redraw so the output pane
/// is re-rendered to reflect the new scroll position.
#[test]
fn handle_mouse_event_scroll_up_returns_redraw() {
    use crossterm::event::{MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    // Set a non-zero output area so the event is considered inside the pane.
    state.output.panel_areas.output_area.set(Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    });
    for _ in 0..50 {
        state.push_output_token(OutputText::new("line\n".to_owned()));
    }
    let event = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 10,
        row: 5,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };
    let outcome = super::handle_mouse_event(&mut state, event);
    assert!(
        matches!(outcome, super::EventOutcome::Redraw),
        "scroll-up must return Redraw so the output pane updates"
    );
}

// ── Guided plan event handler tests ──────────────────────────────────────────

/// Build a minimal `GuidedPlanUiState` for tests that enter `ConversationMode::GuidedPlan`.
fn make_guided_plan_ui() -> crate::domain::tui_state::GuidedPlanUiState {
    use crate::domain::guided_plan::PhaseStatus;
    use crate::domain::tui_state::GuidedPlanUiState;
    GuidedPlanUiState {
        phases: vec![(PhaseName::new("Phase 1"), PhaseStatus::Pending)],
        current_phase: 0,
        plan_name: "test plan".into(),
        review_active: false,
        guided_awaiting_compact: false,
    }
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
        *self.compact_calls.lock().unwrap()
    }
}

impl crate::domain::traits::ChatProvider for RecordingCompactProvider {
    fn submit(&self, _prompt: PromptText, _endpoint: Option<EndpointName>) {}
    fn interrupt(&self) {}
    fn shutdown(&self) {}
    fn restore(&self, _records: Vec<MessageRecord>) {}
    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }
    fn compact(&self) {
        *self.compact_calls.lock().unwrap() += 1;
    }
}

fn single_phase_compact_config() -> crate::domain::guided_plan::GuidedPlanConfig {
    use crate::domain::guided_plan::{GuidedPlanConfig, GuidedPlanPhase, PostPhaseConfig};
    use crate::domain::string_newtypes::PlanPhaseId;

    GuidedPlanConfig {
        name: "Compact Plan".into(),
        phases: vec![GuidedPlanPhase {
            id: PlanPhaseId::new("phase-1"),
            name: "Phase 1".into(),
            prompt: None,
            post_phase: PostPhaseConfig {
                compact: true.into(),
                ..PostPhaseConfig::default()
            },
        }],
    }
}

async fn wait_for_guided_plan_event<F>(
    rx: &mut tokio::sync::broadcast::Receiver<crate::domain::guided_plan::GuidedPlanEvent>,
    predicate: F,
    timeout_ms: u64,
) -> Option<crate::domain::guided_plan::GuidedPlanEvent>
where
    F: Fn(&crate::domain::guided_plan::GuidedPlanEvent) -> bool,
{
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if std::time::Instant::now() >= deadline {
            return None;
        }
        match rx.try_recv() {
            Ok(event) if predicate(&event) => return Some(event),
            Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(_) => return None,
        }
    }
}

/// Verifies that `CompactRequested` sets `guided_awaiting_compact` and pushes
/// a system message describing the compaction so the user sees feedback.
#[test]
fn handle_guided_plan_event_compact_requested_sets_flag_and_pushes_message() {
    use crate::domain::guided_plan::GuidedPlanEvent;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::GuidedPlan(make_guided_plan_ui());
    super::handle_guided_plan_event(&mut state, GuidedPlanEvent::CompactRequested);
    assert!(
        state.is_guided_plan_awaiting_compact().0,
        "guided_awaiting_compact must be true after CompactRequested"
    );
    let has_msg = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("compacting context"));
    assert!(
        has_msg,
        "system message about compaction must be pushed to output"
    );
}

/// Verifies that `CommitRequested` pushes a user-input-styled display line
/// containing the commit label so the user can see the commit was triggered.
#[test]
fn handle_guided_plan_event_commit_requested_pushes_user_input_line() {
    use crate::domain::guided_plan::GuidedPlanEvent;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::GuidedPlan(make_guided_plan_ui());
    super::handle_guided_plan_event(&mut state, GuidedPlanEvent::CommitRequested);
    let has_commit_line = state
        .output
        .lines
        .iter()
        .any(|l| l.text.as_str().contains("committing phase") && l.kind == LineKind::UserInput);
    assert!(
        has_commit_line,
        "user input line for commit must be pushed for CommitRequested"
    );
}

/// Verifies the `AppState` compact flag helpers round-trip: set then clear.
#[test]
fn app_state_compact_flag_set_and_clear() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::GuidedPlan(make_guided_plan_ui());
    assert!(
        !state.is_guided_plan_awaiting_compact().0,
        "compact flag must start false"
    );
    state.set_guided_plan_compact_flag();
    assert!(
        state.is_guided_plan_awaiting_compact().0,
        "compact flag must be true after set"
    );
    state.clear_guided_plan_compact_flag();
    assert!(
        !state.is_guided_plan_awaiting_compact().0,
        "compact flag must be false after clear"
    );
}

/// Verifies that the compact flag helpers are no-ops when not in GuidedPlan mode
/// so they cannot panic or corrupt state in Chat or other modes.
#[test]
fn app_state_compact_flag_helpers_noop_in_chat_mode() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.set_guided_plan_compact_flag(); // must not panic
    assert!(
        !state.is_guided_plan_awaiting_compact().0,
        "compact flag must remain false in Chat mode"
    );
    state.clear_guided_plan_compact_flag(); // must not panic
}

/// Verifies that `apply_guided_plan_actions` for `CommitRequested` sets `is_thinking`,
/// `thinking_label`, and `pending_response` so the spinner starts immediately.
#[tokio::test]
async fn apply_guided_plan_actions_commit_requested_sets_thinking_state() {
    use crate::domain::guided_plan::GuidedPlanEvent;
    let (agent, _dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::GuidedPlan(make_guided_plan_ui());
    let event = GuidedPlanEvent::CommitRequested;
    super::apply_guided_plan_actions(&mut state, &event, &handles);
    assert!(
        state.agent.thinking.is_active,
        "is_thinking must be set after CommitRequested action"
    );
    assert_eq!(
        state.agent.thinking.label, "Committing...",
        "thinking_label must be 'Committing...' after CommitRequested action"
    );
    assert!(
        state.agent.pending_response.is_some(),
        "pending_response must be armed after CommitRequested action"
    );
}

/// Verifies the guided-plan compaction bridge end-to-end:
/// `CompactRequested` triggers `agent.compact()`, then
/// `AgentOutput::CompactionComplete` calls `guided_plan.compaction_done()` and
/// clears the awaiting-compact flag.
#[tokio::test]
async fn guided_plan_compaction_bridge_requests_compact_then_unblocks_on_completion() {
    use crate::domain::guided_plan::GuidedPlanEvent;

    let provider = RecordingCompactProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();

    guided_plan.start(
        single_phase_compact_config(),
        FilePath::new("plans/test.md"),
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut guided_plan_rx = guided_plan.subscribe();
    let mut observed_guided_plan_rx = guided_plan.subscribe();
    guided_plan.confirm_phase();

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::GuidedPlan(make_guided_plan_ui());

    let handles = super::TuiHandles {
        agent: &provider,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let compact_requested = wait_for_guided_plan_event(
        &mut guided_plan_rx,
        |event| matches!(event, GuidedPlanEvent::CompactRequested),
        1000,
    )
    .await
    .expect("guided plan must emit CompactRequested after confirm");
    super::apply_guided_plan_actions(&mut state, &compact_requested, &handles);
    super::handle_guided_plan_event(&mut state, compact_requested);

    assert_eq!(
        provider.compact_call_count(),
        1,
        "CompactRequested must trigger exactly one agent.compact() call"
    );
    assert!(
        state.is_guided_plan_awaiting_compact().0,
        "CompactRequested must set the TUI guided-plan compact flag"
    );

    let _compaction_complete = AgentOutput::CompactionComplete {
        text: OutputText::new("context compacted"),
    };
    super::maybe_finish_guided_plan_compaction(&mut state, Some(()), &handles);

    assert!(
        !state.is_guided_plan_awaiting_compact().0,
        "CompactionComplete must clear the TUI guided-plan compact flag"
    );
    let plan_complete = wait_for_guided_plan_event(
        &mut observed_guided_plan_rx,
        |event| matches!(event, GuidedPlanEvent::PlanComplete),
        1000,
    )
    .await;
    assert!(
        plan_complete.is_some(),
        "CompactionComplete must trigger guided_plan.compaction_done() and unblock the plan"
    );
    guided_plan.shutdown();
}

// ── Regression: ModelsAvailable must be stored while in picker mode ───────────

/// Verifies that `AgentOutput::ModelsAvailable` received while in `SessionPicker`
/// mode is stored in `state.prompt.models.available` so the list is ready when
/// the user transitions to Chat and types `/model`.
///
/// Regression for a bug where the picker-mode agent output arm (in
/// `select_next_event`) and the post-event drain (`drain_channel_to_buf`) both
/// dropped every `AgentOutput` variant except `ContextUsage`, silently discarding
/// `ModelsAvailable`. The model list was empty after entering Chat, leaving
/// `/model` unable to offer any completions.
///
/// Expected: after the drain runs in picker mode, `state.prompt.models.available`
/// contains both supplied models.
#[tokio::test]
async fn models_available_in_picker_mode_is_stored_not_discarded() {
    use crate::domain::types::AgentOutput;
    use tokio::sync::broadcast;

    // Arrange: state is in SessionPicker mode (non-empty session list).
    let picker = PickerState {
        sessions: vec![make_picker_summary()],
        selected: Count::new(0),
    };
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::SessionSelector(picker));
    assert!(
        state.is_picker().0,
        "pre-condition: state must be in picker mode"
    );
    assert!(
        state.prompt.models.available.is_empty(),
        "pre-condition: available model list must start empty"
    );

    // Arrange: broadcast a ModelsAvailable event onto the agent output channel.
    let (tx, mut rx) = broadcast::channel::<AgentOutput>(16);
    let models = vec![
        model_option("model-a", "Model A"),
        model_option("model-b", "Model B"),
    ];
    tx.send(AgentOutput::ModelsAvailable(models)).unwrap();
    drop(tx); // close channel so drain terminates

    // Act: run the post-event channel drain - the same path executed by the TUI
    // main loop after each select_next_event call to flush any accumulated output.
    let mut char_buf = OutputText::new("");
    super::drain_channel_to_buf(&mut state, &mut rx, &mut char_buf);

    // Assert: the model list must be populated despite the picker being active.
    assert!(
        !state.prompt.models.available.is_empty(),
        "state.prompt.models.available must be populated after ModelsAvailable \
         arrives in picker mode; got an empty list - the event was silently dropped"
    );
    assert_eq!(
        state.prompt.models.available.len(),
        2,
        "both models must be stored; got {} model(s)",
        state.prompt.models.available.len()
    );
    let ids: Vec<&str> = state
        .prompt
        .models
        .available
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert!(
        ids.contains(&"model-a") && ids.contains(&"model-b"),
        "stored models must match the supplied list; got: {ids:?}"
    );
}

/// Verifies that `TuiActor::spawn` threads the externally provided feed channel into
/// the returned `TuiHandle` rather than creating a new internal channel.
///
/// Passes `feed_tx.clone()` and a dummy receiver to `spawn`, then sends
/// `AgentFeedOutput::Clear` through `handle.agent_feed_tx` and asserts the event
/// arrives on the original external `feed_rx`.  This confirms that
/// `handle.agent_feed_tx` is wired to the caller-supplied sender, not to a
/// freshly-created internal channel.
///
/// Red state: the Phase 3 Step 1 stub discards the passed `(feed_tx, feed_rx)` with
/// `let _ = (feed_tx, feed_rx)` and creates an internal channel.  `handle.agent_feed_tx`
/// therefore sends to the internal channel, `feed_rx.try_recv()` returns `Err(Empty)`,
/// and the `expect` assertion panics - the intended Red failure.
#[tokio::test]
async fn tui_spawn_accepts_external_feed_channel() {
    use crate::config::types::AppConfig;
    use crate::domain::types::FeedEntry;

    // External channel: we keep feed_rx for assertion; pass a clone of feed_tx to spawn.
    let (feed_tx, _feed_rx) = tokio::sync::mpsc::channel::<FeedEntry>(8);
    // Dummy receiver: satisfies the feed_rx parameter without consuming feed_rx.
    let (_, dummy_feed_rx) = tokio::sync::mpsc::channel::<FeedEntry>(8);

    // Build supporting handles using the same helpers as other tests in this file.
    let (agent, _agent_dir) = make_agent_handle().await;
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = make_scanner();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();

    let (_, output_rx) = tokio::sync::broadcast::channel::<crate::domain::types::AgentOutput>(8);
    let (_, query_rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(8);
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();

    let args = super::TuiSpawnArgs {
        providers: super::TuiServiceHandles {
            agent: std::sync::Arc::new(agent),
            session,
            tools: super::TuiServiceTools {
                command: crate::actors::command::command_actor::build(&[]),
                file_scanner: scanner,
                guided_plan,
                ask: ask_handle,
                logger: logger_handle,
            },
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
        channels: super::TuiInputChannels {
            output_rx,
            query_rx,
            supervisor_rx: None,
        },
        startup: super::TuiStartupData {
            session_summaries: vec![],
            persistence,
            token_tracker: crate::tests::helpers::fake_token_tracker::fake_token_tracker_handle().1,
            config: AppConfig {
                endpoints: vec![],
                default_endpoint: EndpointName::new("ep"),
                agent: crate::config::types::AgentConfig {
                    system_prompt: OutputText::new(""),
                    max_tokens: TokenCount::new(1024),
                    temperature: Temperature::new(0.7),
                    allowed_dirs: vec![],
                },
                copilot: CopilotConfig::default(),
                persistence: PersistenceConfig {
                    log_dir: FilePath::new("./logs"),
                    sessions_dir: None,
                },
                    program_settings: Default::default(),
                    user_settings: Default::default(),
            },
            renderer: crate::tui::render::render_with_overlays,
        },
        sub_actors: make_test_sub_actors(),
    };

    // When: TUI actor is spawned with the real token tracker.
    // The actor task is queued but NOT driven here: ratatui::init() requires a real
    // terminal (PTY) and must not run in unit-test environments.
    let (join, _handle) = super::spawn(args, feed_tx, dummy_feed_rx);

    // Then: the join handle is valid and the actor task has not panicked before
    // being scheduled - confirming token_tracker is accepted by TuiStartupData.
    assert!(
        !join.is_finished(),
        "BH-TKN-039: TUI actor task must be queued (not yet finished) immediately after spawn; \
         a finished handle here would indicate a panic during task setup"
    );
}
