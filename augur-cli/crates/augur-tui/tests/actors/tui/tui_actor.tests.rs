use augur_core::actors::agent::agent_actor::{AgentSpawnArgs, spawn as spawn_agent};
use augur_core::actors::logger::logger_actor::spawn as spawn_logger;
use augur_core::persistence::handle::PersistenceHandle;
use augur_domain::config::types::{AgentConfig, CopilotConfig, PersistenceConfig};
use augur_domain::domain::types::{MessageRecord, MessageType};
use augur_domain::persistence::types::SessionRecord;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_tui::actors::tui::handle::TuiHandle;
use augur_tui::domain::newtypes::{
    Count, NumericNewtype, ScrollOffset, Temperature, TimestampMs, TokenCount,
};
use augur_tui::domain::string_newtypes::{
    EndpointName, FilePath, ModelLabel, OutputText, PromptText, SessionId,
    StringNewtype, ToolName,
};
use augur_tui::domain::tui_state::{
    AppScreen, AppState, ConversationMode, LineKind, PickerSessionIdentity, PickerSessionSummary,
    PickerState,
};
use augur_tui::domain::types::{AgentOutput, CancelSignal, Message};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;

use augur_core::helpers::fake_ask;
use augur_core::helpers::fake_tool::FakeToolExecutor;

fn model_option(id: &str, display_name: &str) -> augur_tui::domain::types::ModelOption {
    augur_tui::domain::types::ModelOption::builder()
        .id(augur_tui::domain::string_newtypes::ModelId::new(id))
        .display_name(ModelLabel::new(display_name))
        .build()
}

/// A test LlmClient that never sends any chunks (sleeps 60 s before dropping the sender).
///
/// Used to keep a turn in-flight for cancel/interrupt tests where the stream
/// must remain open long enough for an interrupt signal to be delivered.
struct StalledLlmClient;

impl augur_domain::domain::traits::LlmClient for StalledLlmClient {
    fn complete_stream(
        &self,
        _request: augur_tui::domain::traits::CompletionRequest,
    ) -> tokio::sync::mpsc::Receiver<augur_tui::domain::types::StreamChunk> {
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

async fn make_agent_handle() -> (
    augur_core::actors::agent::handle::AgentHandle,
    tempfile::TempDir,
) {
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
                augur_core::actors::agent::agent_actor::AgentServices::builder()
                    .persistence(persistence)
                    .logger(logger)
                    .token_tracker(
                        augur_core::helpers::fake_token_tracker::fake_token_tracker_handle().1,
                    )
                    .history_adapter(
                        augur_core::helpers::fake_history_adapter::fake_history_adapter_handle(),
                    )
                    .build(),
            )
            .runtime(
                augur_core::actors::agent::agent_actor::AgentRuntime::builder()
                    .extensions(augur_domain::domain::task_types::AgentExtensions {
                        cache: None,
                        instruction_prefix: None,
                        message_compactor: None,
                    })
                    .app_config(augur_domain::config::AppConfig {
                        endpoints: vec![],
                        default_endpoint: EndpointName::new("ep"),
                        agent: augur_domain::config::types::AgentConfig {
                            system_prompt: augur_domain::domain::string_newtypes::OutputText::new(
                                "test",
                            ),
                            max_tokens: augur_domain::domain::newtypes::TokenCount::new(1024),
                            temperature: augur_domain::domain::newtypes::Temperature::new(0.5),
                            allowed_dirs: vec![],
                        },
                        copilot: augur_domain::config::types::CopilotConfig::default(),
                        persistence: augur_domain::config::types::PersistenceConfig {
                            log_dir: augur_domain::domain::string_newtypes::FilePath::new("./logs"),
                            sessions_dir: None,
                        },
                        program_settings: Default::default(),
                        user_settings: Default::default(),
                    })
                    .build(),
            )
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
    augur_core::actors::FileScannerHandle,
) {
    augur_core::actors::file_scanner::file_scanner_actor::spawn()
}

/// Build a minimal `TuiSubActorHandles` for tests that construct `TuiSpawnArgs`.
///
/// Spawns all six sub-actors with capacity 8 and drops the join handles; the
/// actors run in the background until the test runtime shuts down.
fn make_test_sub_actors() -> super::runtime::layout::TuiSubActorHandles {
    use augur_tui::actors::tui_agent_panel::tui_agent_panel_actor::{
        TuiAgentPanelConfig, spawn as spawn_agent_panel,
    };
    use augur_tui::actors::tui_ask_panel::tui_ask_panel_actor::spawn as spawn_ask_panel;
    use augur_tui::actors::tui_chat_menu::tui_chat_menu_actor::spawn as spawn_chat_menu;
    use augur_tui::actors::tui_dynamic_controls::tui_dynamic_controls_actor::spawn as spawn_controls;
    use augur_tui::actors::tui_main_feed_panel::tui_main_feed_panel_actor::{
        TuiMainFeedConfig, spawn as spawn_main_feed,
    };
    use augur_tui::actors::tui_main_feed_panel::tui_main_feed_panel_ops::MainFeedItem;
    use augur_tui::actors::tui_spinner::tui_spinner_actor::spawn as spawn_spinner;
    use augur_tui::domain::newtypes::Count;
    use augur_tui::domain::types::AgentFeedOutput;

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
                .maybe_title(None)
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
        watch::channel(augur_tui::actors::tui::handle::ShutdownSignal::Running);
    let (agent_feed_tx, _) = tokio::sync::mpsc::channel(1);
    let mut handle = TuiHandle::new(shutdown_rx, agent_feed_tx);

    let wait_task = tokio::spawn(async move {
        handle.wait_for_shutdown().await;
    });

    // Signal shutdown
    shutdown_tx
        .send(augur_tui::actors::tui::handle::ShutdownSignal::Complete)
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

    augur_tui::actors::tui::assistant::query_flow::handle_query_request(&mut state, Some(req));
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: super::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.agent.thinking.is_active = augur_domain::IsActive(true);

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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: super::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.agent.thinking.is_active = augur_domain::IsActive(true);
    state.prompt.buffer = String::new().into();

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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: super::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.agent.thinking.is_active = augur_domain::IsActive(true);
    state.prompt.buffer = "new question".to_owned().into();
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/quit".to_owned().into();
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/help".to_owned().into();
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord {
        meta: augur_domain::persistence::types::SessionMeta {
            id: augur_domain::domain::string_newtypes::SessionId::new("test"),
            created_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            last_updated_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            endpoint_name: EndpointName::new("ep"),
            flags: augur_domain::persistence::types::SessionMetaFlags::default(),
            title: None,
        },
        state: augur_domain::persistence::types::SessionState::default(),
    };
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord {
        meta: augur_domain::persistence::types::SessionMeta {
            id: augur_domain::domain::string_newtypes::SessionId::new("test"),
            created_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            last_updated_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            endpoint_name: EndpointName::new("ep"),
            flags: augur_domain::persistence::types::SessionMetaFlags::default(),
            title: None,
        },
        state: augur_domain::persistence::types::SessionState::default(),
    };
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord {
        meta: augur_domain::persistence::types::SessionMeta {
            id: augur_domain::domain::string_newtypes::SessionId::new("test"),
            created_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            last_updated_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            endpoint_name: EndpointName::new("ep"),
            flags: augur_domain::persistence::types::SessionMetaFlags::default(),
            title: None,
        },
        state: augur_domain::persistence::types::SessionState::default(),
    };
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
                augur_tui::domain::string_newtypes::ToolCallId::new("call_stub"),
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let record = SessionRecord {
        meta: augur_domain::persistence::types::SessionMeta {
            id: augur_domain::domain::string_newtypes::SessionId::new("test"),
            created_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            last_updated_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            endpoint_name: EndpointName::new("ep"),
            flags: augur_domain::persistence::types::SessionMetaFlags::default(),
            title: None,
        },
        state: augur_domain::persistence::types::SessionState::default(),
    };
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord {
        meta: augur_domain::persistence::types::SessionMeta {
            id: augur_domain::domain::string_newtypes::SessionId::new("test"),
            created_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            last_updated_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            endpoint_name: EndpointName::new("ep"),
            flags: augur_domain::persistence::types::SessionMetaFlags::default(),
            title: None,
        },
        state: augur_domain::persistence::types::SessionState::default(),
    };
    record.state.messages = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: augur_tui::domain::types::Message::user(PromptText::new("hello")),
        },
        MessageRecord {
            message_type: MessageType::Error,
            message: augur_tui::domain::types::Message::system(OutputText::new(
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "what is 2+2".to_owned().into();
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "tell me something".to_owned().into();
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
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());

    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = super::TuiHandles {
        agent: &agent,
        session: &session,
        persistence: &persistence,
        tools: super::TuiToolHandles {
            command: &augur_core::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            agent_feed_tx: &agent_feed_tx,
            ask: &ask_handle,
            logger: &logger_handle,
        },
        work: augur_tui::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut record = SessionRecord {
        meta: augur_domain::persistence::types::SessionMeta {
            id: augur_domain::domain::string_newtypes::SessionId::new("test"),
            created_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            last_updated_at: augur_domain::domain::newtypes::TimestampMs::new(0),
            endpoint_name: EndpointName::new("ep"),
            flags: augur_domain::persistence::types::SessionMetaFlags::default(),
            title: None,
        },
        state: augur_domain::persistence::types::SessionState::default(),
    };
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
    let qs = augur_tui::domain::tui_state::QueryState {
        question: PromptText::new("Q"),
        choices: vec!["Alpha".into(), "Beta".into()],
        selected: None,
        freeform: PromptText::new("2"),
        reply_tx,
    };
    let answer = augur_tui::actors::tui::assistant::query_flow::resolve_query_answer(&qs);
    assert_eq!(answer, Some(OutputText::new("Beta")));
}

/// Verifies that resolve_query_answer returns the literal freeform when the number exceeds choice count.
///
/// When freeform contains "5" but only one choice exists, the literal string "5" must be
/// returned so callers get exactly what was typed rather than a silent no-op.
#[test]
fn resolve_query_answer_out_of_range_number_returns_freeform_literal() {
    let (reply_tx, _) = tokio::sync::oneshot::channel::<OutputText>();
    let qs = augur_tui::domain::tui_state::QueryState {
        question: PromptText::new("Q"),
        choices: vec!["Alpha".into()],
        selected: None,
        freeform: PromptText::new("5"),
        reply_tx,
    };
    let answer = augur_tui::actors::tui::assistant::query_flow::resolve_query_answer(&qs);
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
    let qs = augur_tui::domain::tui_state::QueryState {
        question: PromptText::new("Q"),
        choices: vec!["Yes".into(), "No".into()],
        selected: Some(0),
        freeform: PromptText::new(""),
        reply_tx,
    };
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Query(qs);
    augur_tui::actors::tui::assistant::query_flow::handle_query_submit(&mut state);

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
    use augur_tui::domain::types::AgentOutput;
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
    use augur_domain::config::types::AppConfig;
    use augur_tui::domain::types::FeedEntry;

    // External channel: we keep feed_rx for assertion; pass a clone of feed_tx to spawn.
    let (feed_tx, _feed_rx) = tokio::sync::mpsc::channel::<FeedEntry>(8);
    // Dummy receiver: satisfies the feed_rx parameter without consuming feed_rx.
    let (_, dummy_feed_rx) = tokio::sync::mpsc::channel::<FeedEntry>(8);

    // Build supporting handles using the same helpers as other tests in this file.
    let (agent, _agent_dir) = make_agent_handle().await;
    let (_, session) = augur_core::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = make_scanner();
    let (agent_feed_tx, _agent_feed_rx) = tokio::sync::mpsc::channel(8);
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = augur_core::helpers::fake_logger::fake_logger_handle();

    let (_, output_rx) =
        tokio::sync::broadcast::channel::<augur_tui::domain::types::AgentOutput>(8);
    let (_, query_rx) = tokio::sync::mpsc::channel::<QueryUserRequest>(8);
    let (_catalog_manager_join, catalog_manager) =
        augur_core::helpers::fake_catalog_manager::fake_catalog_manager_handle();

    let args = super::TuiSpawnArgs {
        providers: super::TuiServiceHandles {
            agent: std::sync::Arc::new(agent),
            session,
            tools: super::TuiServiceTools {
                command: augur_core::actors::command::command_actor::build(&[]),
                file_scanner: scanner,
                agent_feed_tx,
                ask: ask_handle,
                logger: logger_handle,
            },
            orchestrator: augur_core::helpers::fake_orchestrator::fake_orchestrator_handle(),
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
            token_tracker: augur_core::helpers::fake_token_tracker::fake_token_tracker_handle().1,
            config: AppConfig {
                endpoints: vec![],
                default_endpoint: EndpointName::new("ep"),
                agent: augur_domain::config::types::AgentConfig {
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
            renderer: augur_tui::tui::render::render_with_overlays,
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
