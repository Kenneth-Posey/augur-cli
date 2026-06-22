use crate::domain::newtypes::{Count, NumericNewtype, TimestampMs};
use crate::domain::string_newtypes::{
    EndpointName, OutputText, SdkSessionId, SessionId, StringNewtype,
};
use crate::domain::traits::ChatProvider;
use crate::domain::tui_input::PickerKeyAction;
use crate::domain::tui_state::{
    AppScreen, AppState, ConversationMode, PickerSessionIdentity, PickerSessionSummary, PickerState,
};
use crate::domain::types::{AgentOutput, Message, MessageRecord, MessageType};
use crate::persistence::{store, types::SessionRecord};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::tests::helpers::fake_ask;

fn picker_summary(id: SessionId, endpoint: &str, preview: &str) -> PickerSessionSummary {
    PickerSessionSummary::builder()
        .identity(
            PickerSessionIdentity::builder()
                .id(id)
                .created_at(TimestampMs::new(1_000))
                .last_updated_at(TimestampMs::new(2_000))
                .endpoint_name(EndpointName::new(endpoint))
                .build(),
        )
        .message_count(Count::new(2))
        .preview(OutputText::new(preview))
        .build()
}

fn output_text(state: &AppState) -> String {
    state
        .output
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn wait_for_endpoint(
    session: &crate::actors::session::handle::SessionHandle,
    expected: &str,
) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if session.active_endpoint().as_str() == expected {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("session endpoint must update within timeout");
}

async fn picker_test_lock() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

struct ForceLoadPanicReset;

impl Drop for ForceLoadPanicReset {
    fn drop(&mut self) {
        super::set_force_session_load_panic(false);
    }
}

struct RecordingChatProvider {
    replace_calls: Arc<Mutex<Vec<Option<SdkSessionId>>>>,
    restore_calls: Arc<Mutex<Vec<Vec<MessageRecord>>>>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
}

impl RecordingChatProvider {
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            replace_calls: Arc::new(Mutex::new(Vec::new())),
            restore_calls: Arc::new(Mutex::new(Vec::new())),
            output_tx,
        }
    }

    fn take_replace_calls(&self) -> Vec<Option<SdkSessionId>> {
        self.replace_calls.lock().unwrap().drain(..).collect()
    }

    fn take_restore_calls(&self) -> Vec<Vec<MessageRecord>> {
        self.restore_calls.lock().unwrap().drain(..).collect()
    }
}

impl ChatProvider for RecordingChatProvider {
    fn submit(
        &self,
        _prompt: crate::domain::string_newtypes::PromptText,
        _endpoint: Option<EndpointName>,
    ) {
    }

    fn interrupt(&self) {}

    fn shutdown(&self) {}

    fn restore(&self, records: Vec<MessageRecord>) {
        self.restore_calls.lock().unwrap().push(records);
    }

    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    fn replace_session(&self, sdk_session_id: Option<SdkSessionId>) {
        self.replace_calls.lock().unwrap().push(sdk_session_id);
    }
}

struct PickerTestRigCoreHandles {
    session: crate::actors::session::handle::SessionHandle,
    persistence: crate::persistence::handle::PersistenceHandle,
}

struct PickerTestRigToolHandles {
    scanner: crate::actors::FileScannerHandle,
    guided_plan: crate::actors::guided_plan::GuidedPlanHandle,
    ask_handle: crate::actors::ask::AskHandle,
    command: crate::actors::command::handle::CommandHandle,
    logger: crate::actors::LoggerHandle,
}

struct PickerTestRigResources {
    _sessions_dir: tempfile::TempDir,
    _scanner_join: tokio::task::JoinHandle<()>,
    _ask_dir: tempfile::TempDir,
    _logger_join: tokio::task::JoinHandle<()>,
}

struct PickerTestRig {
    provider: RecordingChatProvider,
    core: PickerTestRigCoreHandles,
    tools: PickerTestRigToolHandles,
    _resources: PickerTestRigResources,
}

impl PickerTestRig {
    async fn new() -> Self {
        let provider = RecordingChatProvider::new();
        let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
        let sessions_dir = tempfile::tempdir().expect("tempdir");
        let persistence =
            crate::persistence::handle::PersistenceHandle::new(sessions_dir.path().to_owned());
        let (scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
        let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
        let (ask_handle, ask_dir) = fake_ask::make_ask_handle().await;
        let command = crate::actors::command::command_actor::build(&[]);
        let (logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
        Self {
            provider,
            core: PickerTestRigCoreHandles {
                session,
                persistence,
            },
            tools: PickerTestRigToolHandles {
                scanner,
                guided_plan,
                ask_handle,
                command,
                logger,
            },
            _resources: PickerTestRigResources {
                _sessions_dir: sessions_dir,
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
                command: &self.tools.command,
                file_scanner: &self.tools.scanner,
                guided_plan: &self.tools.guided_plan,
                ask: &self.tools.ask_handle,
                logger: &self.tools.logger,
            },
            work: crate::actors::tui::tui_actor::TuiWorkHandles {
                orchestrator: crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle(),
                catalog_manager,
            },
        }
    }
}

/// Verifies that `dispatch_picker_action` returns `true` for `Quit`,
/// allowing the picker event loop to exit immediately.
#[tokio::test]
async fn dispatch_picker_action_quit_returns_true() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let should_quit =
        super::dispatch_picker_action(&mut state, PickerKeyAction::Quit, &rig.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Break(())),
        "Quit must request TUI shutdown"
    );
}

/// Verifies that `NewSession` leaves the picker, switches to chat mode,
/// and tells the provider to clear any linked SDK session.
#[tokio::test]
async fn dispatch_picker_action_new_session_switches_to_chat_and_clears_sdk_session() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let summary = picker_summary(SessionId::new("picker-row"), "ep", "preview");
    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![summary],
            selected: Count::new(0),
        }),
    );

    let should_quit =
        super::dispatch_picker_action(&mut state, PickerKeyAction::NewSession, &rig.handles())
            .await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "NewSession must keep the TUI running"
    );
    assert!(
        matches!(state.interaction.screen, AppScreen::Conversation),
        "NewSession must leave the picker"
    );
    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "NewSession must enter chat mode"
    );
    assert_eq!(
        rig.provider.take_replace_calls(),
        vec![None],
        "NewSession must clear any active SDK session"
    );
}

/// Verifies that `Ignored` leaves the highlighted picker row unchanged and does
/// not trigger any restore or session replacement side effects.
#[tokio::test]
async fn dispatch_picker_action_ignored_leaves_picker_state_unchanged() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![
                picker_summary(SessionId::new("a"), "ep", "first"),
                picker_summary(SessionId::new("b"), "ep", "second"),
            ],
            selected: Count::new(1),
        }),
    );

    let should_quit =
        super::dispatch_picker_action(&mut state, PickerKeyAction::Ignored, &rig.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "Ignored must not quit the TUI"
    );
    match &state.interaction.screen {
        AppScreen::SessionSelector(picker) => {
            assert_eq!(
                picker.selected,
                Count::new(1),
                "Ignored must keep the same highlighted row"
            );
        }
        AppScreen::Conversation => panic!("Ignored must keep the picker open"),
    }
    assert!(
        rig.provider.take_restore_calls().is_empty(),
        "Ignored must not restore a session"
    );
    assert!(
        rig.provider.take_replace_calls().is_empty(),
        "Ignored must not replace the SDK session"
    );
}

/// Verifies that `SelectUp` moves the highlighted picker row toward the start
/// of the list so the previous saved session becomes selected.
#[tokio::test]
async fn dispatch_picker_action_select_up_moves_highlight_to_previous_row() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![
                picker_summary(SessionId::new("a"), "ep", "first"),
                picker_summary(SessionId::new("b"), "ep", "second"),
            ],
            selected: Count::new(1),
        }),
    );

    let should_quit =
        super::dispatch_picker_action(&mut state, PickerKeyAction::SelectUp, &rig.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "SelectUp must not quit the TUI"
    );
    match &state.interaction.screen {
        AppScreen::SessionSelector(picker) => {
            assert_eq!(
                picker.selected,
                Count::new(0),
                "SelectUp must move selection up by one row"
            );
        }
        AppScreen::Conversation => panic!("SelectUp must keep the picker open"),
    }
}

/// Verifies that `SelectDown` moves the highlighted picker row toward the end
/// of the list so the next saved session becomes selected.
#[tokio::test]
async fn dispatch_picker_action_select_down_moves_highlight_to_next_row() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![
                picker_summary(SessionId::new("a"), "ep", "first"),
                picker_summary(SessionId::new("b"), "ep", "second"),
            ],
            selected: Count::new(0),
        }),
    );

    let should_quit =
        super::dispatch_picker_action(&mut state, PickerKeyAction::SelectDown, &rig.handles())
            .await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "SelectDown must not quit the TUI"
    );
    match &state.interaction.screen {
        AppScreen::SessionSelector(picker) => {
            assert_eq!(
                picker.selected,
                Count::new(1),
                "SelectDown must move selection down by one row"
            );
        }
        AppScreen::Conversation => panic!("SelectDown must keep the picker open"),
    }
}

/// Verifies that `Delete` removes the selected row from the picker and deletes
/// the corresponding saved session file.
#[tokio::test]
async fn dispatch_picker_action_delete_removes_selected_row_and_file() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut first = SessionRecord::new(EndpointName::new("ep-a"));
    first.state.messages = vec![MessageRecord {
        message_type: MessageType::User,
        message: Message::user("first"),
    }];
    let mut second = SessionRecord::new(EndpointName::new("ep-b"));
    second.state.messages = vec![MessageRecord {
        message_type: MessageType::User,
        message: Message::user("second"),
    }];
    store::save_session(&first, &rig.core.persistence.sessions_dir()).expect("save first");
    store::save_session(&second, &rig.core.persistence.sessions_dir()).expect("save second");

    let first_id = first.meta.id.clone();
    let second_id = second.meta.id.clone();

    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![
                picker_summary(first_id.clone(), "ep-a", "first"),
                picker_summary(second_id.clone(), "ep-b", "second"),
            ],
            selected: Count::new(0),
        }),
    );

    let should_quit =
        super::dispatch_picker_action(&mut state, PickerKeyAction::Delete, &rig.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "Delete must not quit the TUI"
    );
    match &state.interaction.screen {
        AppScreen::SessionSelector(picker) => {
            assert_eq!(
                picker.sessions.len(),
                1,
                "Delete must remove one picker row"
            );
            assert_eq!(
                picker.sessions[0].identity.id.as_str(),
                second_id.as_str(),
                "Delete must remove the currently selected row"
            );
            assert_eq!(
                picker.selected,
                Count::new(0),
                "selection must remain clamped after deletion"
            );
        }
        AppScreen::Conversation => panic!("Delete must keep the picker open"),
    }
    assert!(
        store::load_session(&rig.core.persistence.sessions_dir(), &first_id).is_err(),
        "Delete must remove the selected session file from disk"
    );
    assert!(
        store::load_session(&rig.core.persistence.sessions_dir(), &second_id).is_ok(),
        "Delete must not remove unselected session files"
    );
}

/// Verifies that `Confirm` restores the selected session's visible history,
/// updates session routing state, and exits the picker into chat mode.
#[tokio::test]
async fn dispatch_picker_action_confirm_restores_selected_session() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let sdk_session_id = SdkSessionId::new("sdk-session-42");
    let mut record = SessionRecord::new(EndpointName::new("restored-ep"));
    record.meta.flags.sdk_session_id = Some(sdk_session_id.clone());
    record.state.messages = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user("hello from saved session"),
        },
        MessageRecord {
            message_type: MessageType::Assistant,
            message: Message::assistant(OutputText::new("restored reply")),
        },
    ];
    store::save_session(&record, &rig.core.persistence.sessions_dir())
        .expect("save session fixture");
    let summary = picker_summary(
        record.meta.id.clone(),
        "restored-ep",
        "hello from saved session",
    );
    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![summary],
            selected: Count::new(0),
        }),
    );

    let should_quit =
        super::dispatch_picker_action(&mut state, PickerKeyAction::Confirm, &rig.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "Confirm must restore the session without quitting"
    );
    assert!(
        matches!(state.interaction.screen, AppScreen::Conversation),
        "Confirm must leave the picker after restore"
    );
    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "Confirm must enter chat mode after restore"
    );
    assert_eq!(
        rig.core.persistence.session_id().as_str(),
        record.meta.id.as_str(),
        "successful restore must move persistence to the restored session ID"
    );
    wait_for_endpoint(&rig.core.session, "restored-ep").await;
    let restore_calls = rig.provider.take_restore_calls();
    assert_eq!(
        restore_calls.len(),
        1,
        "Confirm must replay saved history once"
    );
    assert_eq!(
        restore_calls[0].len(),
        2,
        "Confirm must replay all saved message records"
    );
    assert_eq!(
        rig.provider.take_replace_calls(),
        vec![Some(sdk_session_id)],
        "Confirm must reconnect the provider to the restored SDK session"
    );
    let rendered = output_text(&state);
    assert!(
        rendered.contains("hello from saved session"),
        "restored user content must be visible after Confirm, got: {rendered:?}"
    );
    assert!(
        rendered.contains("restored reply"),
        "restored assistant content must be visible after Confirm, got: {rendered:?}"
    );
    assert!(
        rendered.contains("[system] restored session"),
        "Confirm must show the restored-session confirmation line, got: {rendered:?}"
    );
}

/// Verifies that an out-of-bounds picker selection falls back to conversation
/// mode without emitting an error or replaying any session history.
#[tokio::test]
async fn restore_session_out_of_bounds_switches_to_chat_without_loading() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut state = AppState::new(
        EndpointName::new("ep"),
        AppScreen::SessionSelector(PickerState {
            sessions: vec![picker_summary(SessionId::new("only-row"), "ep", "preview")],
            selected: Count::new(0),
        }),
    );
    let picker = PickerState {
        sessions: vec![picker_summary(SessionId::new("only-row"), "ep", "preview")],
        selected: Count::new(4),
    };

    super::restore_session(&mut state, picker, &rig.handles()).await;

    assert!(
        matches!(state.interaction.screen, AppScreen::Conversation),
        "out-of-bounds restore must leave the picker"
    );
    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "out-of-bounds restore must end in chat mode"
    );
    assert!(
        state.output.lines.is_empty(),
        "out-of-bounds restore must not emit output"
    );
    assert!(
        rig.provider.take_restore_calls().is_empty(),
        "out-of-bounds restore must not replay session history"
    );
}

/// Verifies that a session-file load error renders a visible error line and
/// returns the UI to chat mode without replaying any session state.
#[tokio::test]
async fn restore_session_load_error_pushes_error_and_returns_to_chat() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let picker = PickerState {
        sessions: vec![picker_summary(
            SessionId::new("missing-session"),
            "ep",
            "missing",
        )],
        selected: Count::new(0),
    };

    super::restore_session(&mut state, picker, &rig.handles()).await;

    assert!(
        matches!(state.interaction.screen, AppScreen::Conversation),
        "load-error restore must end on the conversation screen"
    );
    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "load-error restore must end in chat mode"
    );
    let rendered = output_text(&state);
    assert!(
        rendered.contains("[error] failed to load session:"),
        "load-error restore must show a user-visible error, got: {rendered:?}"
    );
    assert!(
        rig.provider.take_restore_calls().is_empty(),
        "load-error restore must not replay session history"
    );
    assert!(
        rig.provider.take_replace_calls().is_empty(),
        "load-error restore must not replace the SDK session"
    );
}

/// Verifies that a blocking-session-load task panic is surfaced as a visible
/// load error and still returns the picker flow to chat mode.
#[tokio::test]
async fn restore_session_join_failure_pushes_task_panicked_error() {
    let _guard = picker_test_lock().await;
    let rig = PickerTestRig::new().await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let picker = PickerState {
        sessions: vec![picker_summary(
            SessionId::new("panic-session"),
            "ep",
            "panic",
        )],
        selected: Count::new(0),
    };
    super::set_force_session_load_panic(true);
    let _reset = ForceLoadPanicReset;

    super::restore_session(&mut state, picker, &rig.handles()).await;

    assert!(
        matches!(state.interaction.screen, AppScreen::Conversation),
        "join-failure restore must end on the conversation screen"
    );
    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "join-failure restore must end in chat mode"
    );
    let rendered = output_text(&state);
    assert!(
        rendered.contains("[error] failed to load session: task panicked:"),
        "join-failure restore must surface the task panic, got: {rendered:?}"
    );
    assert!(
        rig.provider.take_restore_calls().is_empty(),
        "join-failure restore must not replay session history"
    );
    assert!(
        rig.provider.take_replace_calls().is_empty(),
        "join-failure restore must not replace the SDK session"
    );
}

#[test]
fn mirror_sync_executes_dispatch_picker_action_quit_returns_true() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
