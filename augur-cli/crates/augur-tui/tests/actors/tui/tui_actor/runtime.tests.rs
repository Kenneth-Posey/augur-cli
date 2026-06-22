use crate::domain::string_newtypes::{EndpointName, StringNewtype};
use crate::domain::tui_state::{AppScreen, AppState, ConversationMode, GuidedPlanUiState};

fn conversation_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

fn guided_plan_state_awaiting_compact() -> AppState {
    let mut state = conversation_state();
    state.interaction.mode = ConversationMode::GuidedPlan(GuidedPlanUiState {
        phases: vec![],
        current_phase: 0,
        plan_name: "Test Plan".into(),
        review_active: false,
        guided_awaiting_compact: true,
    });
    state
}

// ── TestRig for tests that need TuiHandles ───────────────────────────────────

struct NullChat(tokio::sync::broadcast::Sender<crate::domain::types::AgentOutput>);

impl NullChat {
    fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(1);
        Self(tx)
    }
}

impl crate::domain::traits::ChatProvider for NullChat {
    fn submit(
        &self,
        _: crate::domain::string_newtypes::PromptText,
        _: Option<crate::domain::string_newtypes::EndpointName>,
    ) {
    }
    fn interrupt(&self) {}
    fn shutdown(&self) {}
    fn restore(&self, _: Vec<crate::persistence::types::MessageRecord>) {}
    fn subscribe_output(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::domain::types::AgentOutput> {
        self.0.subscribe()
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
    provider: NullChat,
    core: TestRigCoreHandles,
    tools: TestRigToolHandles,
    _resources: TestRigResources,
}

impl TestRig {
    async fn new() -> Self {
        let command = crate::actors::command::command_actor::build(&[]);
        let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
        let dir = tempfile::tempdir().expect("tempdir");
        let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
        let (scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
        let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
        let (ask, ask_dir) = crate::tests::helpers::fake_ask::make_ask_handle().await;
        let (logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
        Self {
            provider: NullChat::new(),
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
                _persistence_dir: dir,
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

// ── configure_terminal_startup ───────────────────────────────────────────────

/// Verifies that `configure_terminal_startup` writes terminal control bytes to
/// the supplied writer and returns `Ok(())`, confirming the escape sequences
/// for mouse capture and bracketed paste are emitted at startup.
#[test]
fn configure_terminal_startup_writes_control_bytes_and_returns_ok() {
    let mut buf: Vec<u8> = Vec::new();
    let result = super::configure_terminal_startup(&mut buf);
    assert!(
        result.is_ok(),
        "configure_terminal_startup must succeed on a Vec<u8> writer"
    );
    assert!(
        !buf.is_empty(),
        "configure_terminal_startup must write terminal escape bytes"
    );
}

// ── maybe_finish_guided_plan_compaction ──────────────────────────────────────

/// Verifies that `maybe_finish_guided_plan_compaction` is a no-op when
/// `is_compaction_done` is `None`, leaving state unchanged.
#[tokio::test]
async fn maybe_finish_guided_plan_compaction_does_nothing_when_compaction_not_done() {
    let rig = TestRig::new().await;
    let mut state = guided_plan_state_awaiting_compact();

    super::maybe_finish_guided_plan_compaction(&mut state, None, &rig.handles());

    // Flag must remain set since no compaction signal was delivered.
    let ConversationMode::GuidedPlan(gs) = &state.interaction.mode else {
        panic!("expected GuidedPlan mode");
    };
    assert!(
        gs.guided_awaiting_compact,
        "guided_awaiting_compact must remain true when is_compaction_done is None"
    );
}

/// Verifies that `maybe_finish_guided_plan_compaction` is a no-op when the
/// interaction mode is Chat rather than GuidedPlan, even when a compaction
/// signal is present.
#[tokio::test]
async fn maybe_finish_guided_plan_compaction_does_nothing_in_chat_mode() {
    let rig = TestRig::new().await;
    let mut state = conversation_state(); // Chat mode, not GuidedPlan

    // Should not panic or change any mode state.
    super::maybe_finish_guided_plan_compaction(&mut state, Some(()), &rig.handles());

    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "mode must remain Chat after a no-op compaction signal"
    );
}

/// Verifies that `maybe_finish_guided_plan_compaction` clears the
/// `guided_awaiting_compact` flag when in GuidedPlan mode and the compaction
/// signal is present, indicating the runtime correctly advances past the
/// compact wait point.
#[tokio::test]
async fn maybe_finish_guided_plan_compaction_clears_flag_when_in_guided_plan_awaiting() {
    let rig = TestRig::new().await;
    let mut state = guided_plan_state_awaiting_compact();

    super::maybe_finish_guided_plan_compaction(&mut state, Some(()), &rig.handles());

    let ConversationMode::GuidedPlan(gs) = &state.interaction.mode else {
        panic!("expected GuidedPlan mode after compaction signal");
    };
    assert!(
        !gs.guided_awaiting_compact,
        "guided_awaiting_compact must be cleared after compaction signal is delivered"
    );
}
