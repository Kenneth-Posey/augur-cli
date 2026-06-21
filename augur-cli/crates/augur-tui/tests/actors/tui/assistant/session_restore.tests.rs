use crate::domain::newtypes::ScrollOffset;
use crate::domain::string_newtypes::{
    EndpointName, ModelLabel, OutputText, PromptText, StringNewtype, TaskName, ToolCallId, ToolName,
};
use crate::domain::tui_state::{AppScreen, AppState, LineKind, SecondaryView};
use crate::domain::types::{Message, ToolCall};
use crate::persistence::types::{MessageRecord, MessageType, SessionRecord};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::tests::helpers::fake_ask;

/// Verifies that hydrate_output_from_messages skips tool messages so only
/// user-visible content (user, assistant, error) appears in the output pane.
#[test]
fn hydrate_output_skips_tool_messages() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let tool = ToolName::new("t");
    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![MessageRecord {
        message_type: MessageType::Tool(ToolName::new("t")),
        message: Message::tool_result(
            crate::domain::string_newtypes::ToolCallId::new("call_stub"),
            &tool,
            OutputText::new("tool output"),
        ),
    }];
    super::hydrate_output_from_messages(&mut state, &record);
    let output_text: String = state
        .output
        .lines
        .iter()
        .map(|l| l.text.as_str().to_owned())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        !output_text.contains("tool output"),
        "tool output must not appear in restored output, got: {output_text:?}"
    );
}

/// Verifies that hydrate_output_from_messages renders System messages in the
/// output pane. System messages mark in-session events (e.g., model switches)
/// and must be visible when the session is restored.
#[test]
fn hydrate_output_renders_system_messages() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![MessageRecord {
        message_type: MessageType::System,
        message: Message::system(OutputText::new("model switched to gpt-4o")),
    }];
    super::hydrate_output_from_messages(&mut state, &record);
    let output_text: String = state
        .output
        .lines
        .iter()
        .map(|l| l.text.as_str().to_owned())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        output_text.contains("model switched to gpt-4o"),
        "system message must appear in restored output, got: {output_text:?}"
    );
}

/// Verifies that restored user slash commands are preserved as user-input lines
/// so Up/Down history navigation can recall them.
#[test]
fn hydrate_output_restores_user_slash_commands_as_user_input() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages = vec![MessageRecord {
        message_type: MessageType::User,
        message: Message::user(PromptText::new("/model gpt-5")),
    }];

    super::hydrate_output_from_messages(&mut state, &record);

    assert!(
        state.output.lines.iter().any(|line| {
            line.kind == LineKind::UserInput && line.text.as_str() == "> /model gpt-5"
        }),
        "restored slash command must appear as UserInput line for history recall"
    );
}

/// Verifies that assistant `tool_calls` are restored as tool-call output rows
/// before assistant text so historical tool invocations remain visible.
#[test]
fn hydrate_output_restores_assistant_tool_calls() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut record = SessionRecord::new(EndpointName::new("ep"));
    let tool_call = ToolCall {
        id: ToolCallId::new("call_file_read"),
        name: ToolName::new("file_read"),
        arguments: serde_json::json!({"path":"/tmp/a.rs"}),
    };
    let assistant =
        Message::assistant_with_tool_calls(OutputText::new("Done reading file."), vec![tool_call]);
    record.state.messages = vec![MessageRecord {
        message_type: MessageType::Assistant,
        message: assistant,
    }];

    super::hydrate_output_from_messages(&mut state, &record);

    assert!(
        state.output.lines.iter().any(|line| {
            line.kind == LineKind::ToolCall && line.text.as_str().contains("file_read: /tmp/a.rs")
        }),
        "assistant tool call must be restored as a visible ToolCall line"
    );
    assert!(
        state
            .output
            .lines
            .iter()
            .any(|line| line.kind == LineKind::Plain
                && line.text.as_str().contains("Done reading file.")),
        "assistant text must still be restored after tool calls"
    );
    let tool_idx = state
        .output
        .lines
        .iter()
        .position(|line| {
            line.kind == LineKind::ToolCall && line.text.as_str().contains("file_read: /tmp/a.rs")
        })
        .expect("tool call line");
    let assistant_idx = state
        .output
        .lines
        .iter()
        .position(|line| {
            line.kind == LineKind::Plain && line.text.as_str().contains("Done reading file.")
        })
        .expect("assistant line");
    assert_eq!(
        assistant_idx,
        tool_idx + 1,
        "restored assistant text must immediately follow tool-call rows without inserted blank gaps"
    );
}

// ── apply_restored_session ────────────────────────────────────────────────────

/// Test double for `ChatProvider` that records `replace_session` calls.
struct SpyChatProvider {
    replace_calls:
        std::sync::Arc<std::sync::Mutex<Vec<Option<crate::domain::string_newtypes::SdkSessionId>>>>,
    output_tx: tokio::sync::broadcast::Sender<crate::domain::types::AgentOutput>,
}

impl SpyChatProvider {
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            replace_calls: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            output_tx,
        }
    }

    fn take_replace_calls(&self) -> Vec<Option<crate::domain::string_newtypes::SdkSessionId>> {
        self.replace_calls.lock().unwrap().drain(..).collect()
    }
}

impl crate::domain::traits::ChatProvider for SpyChatProvider {
    fn submit(
        &self,
        _prompt: crate::domain::string_newtypes::PromptText,
        _endpoint: Option<crate::domain::string_newtypes::EndpointName>,
    ) {
    }
    fn interrupt(&self) {}
    fn shutdown(&self) {}
    fn restore(&self, _records: Vec<crate::persistence::types::MessageRecord>) {}
    fn subscribe_output(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::domain::types::AgentOutput> {
        self.output_tx.subscribe()
    }
    fn replace_session(
        &self,
        sdk_session_id: Option<crate::domain::string_newtypes::SdkSessionId>,
    ) {
        self.replace_calls.lock().unwrap().push(sdk_session_id);
    }
}

/// Verifies that apply_restored_session calls replace_session with the SDK
/// session ID from the loaded record so the Copilot actor reconnects to the
/// original session rather than the one created at startup.
#[tokio::test]
async fn apply_restored_session_calls_replace_session_when_sdk_id_present() {
    use crate::domain::string_newtypes::{SdkSessionId, StringNewtype};
    use crate::persistence::types::SessionRecord;

    let provider = SpyChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = crate::actors::tui::tui_actor::TuiHandles {
        agent: &provider,
        session: &session,
        persistence: &persistence,
        tools: crate::actors::tui::tui_actor::TuiToolHandles {
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

    let sdk_id = SdkSessionId::new("expected-sdk-session-id");
    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.meta.flags.sdk_session_id = Some(sdk_id.clone());

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    let calls = provider.take_replace_calls();
    assert_eq!(
        calls.len(),
        1,
        "replace_session must be called exactly once"
    );
    assert_eq!(
        calls[0].as_ref().map(|id| id.as_str()),
        Some(sdk_id.as_str()),
        "replace_session must receive the SDK session ID from the loaded record"
    );
}

/// Verifies that apply_restored_session calls replace_session with None when
/// the loaded session has no linked SDK session ID, so the actor creates a new
/// session rather than resuming a non-existent one.
#[tokio::test]
async fn apply_restored_session_calls_replace_session_with_none_when_no_sdk_id() {
    use crate::persistence::types::SessionRecord;

    let provider = SpyChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = crate::actors::tui::tui_actor::TuiHandles {
        agent: &provider,
        session: &session,
        persistence: &persistence,
        tools: crate::actors::tui::tui_actor::TuiToolHandles {
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
    // sdk_session_id defaults to None
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    let calls = provider.take_replace_calls();
    assert_eq!(
        calls.len(),
        1,
        "replace_session must be called exactly once"
    );
    assert!(
        calls[0].is_none(),
        "replace_session must receive None when record has no SDK session ID"
    );
}

/// Verifies that apply_restored_session resets scroll_offset to 0 so the
/// conversation is positioned at the bottom (following the latest messages)
/// when a session is restored.
#[tokio::test]
async fn apply_restored_session_resets_scroll_offset() {
    use crate::persistence::types::SessionRecord;

    let provider = SpyChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = crate::actors::tui::tui_actor::TuiHandles {
        agent: &provider,
        session: &session,
        persistence: &persistence,
        tools: crate::actors::tui::tui_actor::TuiToolHandles {
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

    // Simulate a state where scroll_offset is non-zero (scrolled up)
    state.output.scroll_offset.set(ScrollOffset::of(5));

    super::apply_restored_session(&mut state, record, &handles).await;

    assert_eq!(
        state.output.scroll_offset.get(),
        ScrollOffset::of(0),
        "scroll_offset must be reset to 0 when session is restored"
    );
}

/// Verifies that restore + render keeps the same main-conversation message
/// sequence visible whether the background agent panel is open or closed.
///
/// This is a diagnosis test: if the main panel were truncating after the system
/// message, the open-panel render would drop the later user/assistant messages.
#[tokio::test]
async fn apply_restored_session_renders_messages_after_mixed_system_and_error_entries() {
    use crate::domain::string_newtypes::SdkSessionId;

    let provider = SpyChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask_handle, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger_handle) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let handles = crate::actors::tui::tui_actor::TuiHandles {
        agent: &provider,
        session: &session,
        persistence: &persistence,
        tools: crate::actors::tui::tui_actor::TuiToolHandles {
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
    record.meta.flags.sdk_session_id = Some(SdkSessionId::new("sdk-id"));
    let long_assistant = "assistant one ".repeat(140);
    record.state.messages = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user(crate::domain::string_newtypes::PromptText::new(
                "first user",
            )),
        },
        MessageRecord {
            message_type: MessageType::Assistant,
            message: Message::assistant(OutputText::new(long_assistant)),
        },
        MessageRecord {
            message_type: MessageType::System,
            message: Message::system(OutputText::new("[system] model switched to auto")),
        },
        MessageRecord {
            message_type: MessageType::Error,
            message: Message {
                role: crate::domain::types::Role::System,
                content: OutputText::new("[error] restore issue"),
                timestamp: crate::domain::TimestampMs::now(),
                tool_call_id: None,
                tool_calls: None,
            },
        },
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user(crate::domain::string_newtypes::PromptText::new(
                "second user",
            )),
        },
        MessageRecord {
            message_type: MessageType::Assistant,
            message: Message::assistant(OutputText::new("assistant two")),
        },
    ];

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    super::apply_restored_session(&mut state, record, &handles).await;

    let closed = render_main_panel_text(&mut state, None);
    let open = render_main_panel_text(&mut state, Some(SecondaryView::AgentFeed));

    assert_main_sequence_visible(
        &closed,
        "closed-panel render",
        &[
            "model switched to auto",
            "restore issue",
            "second user",
            "assistant two",
        ],
    );
    assert_main_sequence_visible(
        &open,
        "open-panel render",
        &[
            "restore issue",
            "second user",
            "assistant two",
            "restored session",
        ],
    );
}

/// Verifies restoring a session does not hydrate token totals from message history.
#[tokio::test]
async fn apply_restored_session_does_not_hydrate_token_totals() {
    use crate::domain::types::ProjectTokenTotals;
    use crate::persistence::types::SessionRecord;

    let provider = SpyChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();

    let handles = crate::actors::tui::tui_actor::TuiHandles {
        agent: &provider,
        session: &session,
        persistence: &persistence,
        tools: crate::actors::tui::tui_actor::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask,
            logger: &logger,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut record = SessionRecord::new(EndpointName::new("ep"));
    record.state.messages.push(MessageRecord {
        message_type: crate::persistence::types::MessageType::Assistant,
        message: Message::assistant(OutputText::new("hello")),
    });

    super::apply_restored_session(&mut state, record, &handles).await;
    assert_eq!(
        state.status.token_totals,
        ProjectTokenTotals::default(),
        "restore must not hydrate token totals from historical messages"
    );
}

/// Verifies that apply_restored_session reports endpoint-switch failures and
/// stops before replaying the saved history.
#[tokio::test]
async fn apply_restored_session_reports_endpoint_switch_failure() {
    let provider = SpyChatProvider::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let (ask, _ask_dir) = fake_ask::make_ask_handle().await;
    let (_logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
    let (_catalog_manager_join, catalog_manager) =
        crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
    let (session_join, session) =
        crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    session_join.abort();
    let _ = session_join.await;
    let handles = crate::actors::tui::tui_actor::TuiHandles {
        agent: &provider,
        session: &session,
        persistence: &persistence,
        tools: crate::actors::tui::tui_actor::TuiToolHandles {
            command: &crate::actors::command::command_actor::build(&[]),
            file_scanner: &scanner,
            guided_plan: &guided_plan,
            ask: &ask,
            logger: &logger,
        },
        work: crate::actors::tui::tui_actor::TuiWorkHandles {
            orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
            catalog_manager,
        },
    };

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut record = SessionRecord::new(EndpointName::new("other"));
    record.state.messages.push(MessageRecord {
        message_type: MessageType::User,
        message: Message::user(PromptText::new("should not leak")),
    });
    record.state.messages.push(MessageRecord {
        message_type: MessageType::Assistant,
        message: Message::assistant(OutputText::new("should not hydrate")),
    });
    super::apply_restored_session(&mut state, record, &handles).await;

    assert!(
        state.output.lines.iter().any(|line| {
            line.text
                .as_str()
                .contains("failed to restore session endpoint")
        }),
        "restore should surface endpoint switch failure"
    );
    assert!(
        state
            .output
            .lines
            .iter()
            .all(|line| !line.text.as_str().contains("should not")),
        "restore failure should not hydrate transcript"
    );
}

fn render_main_panel_text(state: &mut AppState, secondary_view: Option<SecondaryView>) -> String {
    state.interaction.panel.secondary_view = secondary_view;
    if matches!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::AgentFeed)
    ) {
        state.interaction.panel.agent_feed.active_task = Some(TaskName::new("review"));
        state.interaction.panel.agent_feed.current_agent_model = Some(ModelLabel::new("model"));
    } else {
        state.interaction.panel.agent_feed.active_task = None;
        state.interaction.panel.agent_feed.current_agent_model = None;
    }

    let display = crate::domain::tui_display_state::TuiDisplayState::project_from(state);
    let mut terminal = Terminal::new(TestBackend::new(80, 12)).expect("terminal");
    terminal
        .draw(|frame| {
            crate::tui::components::conversation_container::render_conversation_container(
                frame,
                &display,
                crate::tui::layout::ConversationArea::full(Rect {
                    x: 0,
                    y: 0,
                    width: 80,
                    height: 12,
                }),
            );
        })
        .expect("draw");

    let buf = terminal.backend().buffer();
    (0..12u16)
        .map(|y| {
            (0..80u16)
                .map(|x| {
                    buf.cell((x, y))
                        .map(|cell| cell.symbol().to_owned())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_main_sequence_visible(rendered: &str, label: &str, expected: &[&str]) {
    let mut search_start = 0usize;
    for needle in expected {
        let Some(pos) = rendered[search_start..].find(needle) else {
            panic!(
                "{label} must contain {needle:?} after index {search_start}; rendered={rendered:?}"
            );
        };
        search_start += pos + needle.len();
    }
}
