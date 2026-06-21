use crate::actors::token_tracker;
use crate::config::types::{
    AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials, PersistenceConfig,
    Provider,
};
use crate::domain::newtypes::{Count, NumericNewtype, TimestampMs};
use crate::domain::newtypes::{Temperature, TokenCount};
use crate::domain::string_newtypes::{
    EndpointName, EndpointUrl, FilePath, ModelName, OutputText, SessionId, StringNewtype,
};
use crate::domain::traits::ChatProvider;
use crate::domain::tui_state::{AppScreen, PickerState};
use crate::domain::types::AgentOutput;
use crate::persistence::types::{SessionIdentity, SessionSummary};
use std::sync::{Arc, Mutex};

fn make_summary(id: &str) -> SessionSummary {
    SessionSummary::builder()
        .identity(
            SessionIdentity::builder()
                .id(SessionId::new(id))
                .created_at(TimestampMs::new(0))
                .last_updated_at(TimestampMs::new(0))
                .endpoint_name(EndpointName::new("ep"))
                .build(),
        )
        .message_count(Count::new(3))
        .preview(OutputText::new("hello"))
        .build()
}

fn noop_renderer(
    _: &mut ratatui::Frame<'_>,
    _: &crate::domain::tui_display_state::TuiDisplayState,
) {
}

fn test_config() -> AppConfig {
    AppConfig {
        endpoints: vec![EndpointConfig {
            name: EndpointName::new("ep"),
            provider: Provider::Ollama,
            base_url: EndpointUrl::new("http://localhost:11434"),
            model: ModelName::new("llama3.2"),
            credentials: EndpointCredentials::default(),
        }],
        default_endpoint: EndpointName::new("ep"),
        agent: AgentConfig {
            system_prompt: OutputText::new("sys"),
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
    }
}

struct RecordingChatProvider {
    replace_calls: Arc<Mutex<Vec<Option<crate::domain::string_newtypes::SdkSessionId>>>>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
}

impl RecordingChatProvider {
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            replace_calls: Arc::new(Mutex::new(Vec::new())),
            output_tx,
        }
    }

    fn take_replace_calls(&self) -> Vec<Option<crate::domain::string_newtypes::SdkSessionId>> {
        self.replace_calls.lock().expect("lock").drain(..).collect()
    }
}

impl ChatProvider for RecordingChatProvider {
    fn submit(&self, _: crate::domain::string_newtypes::PromptText, _: Option<EndpointName>) {}

    fn interrupt(&self) {}

    fn shutdown(&self) {}

    fn restore(&self, _: Vec<crate::persistence::types::MessageRecord>) {}

    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    fn replace_session(
        &self,
        sdk_session_id: Option<crate::domain::string_newtypes::SdkSessionId>,
    ) {
        self.replace_calls
            .lock()
            .expect("lock")
            .push(sdk_session_id);
    }
}

struct TestRigCoreHandles {
    session: crate::actors::SessionHandle,
    persistence: crate::persistence::handle::PersistenceHandle,
    token_tracker: crate::actors::TokenTrackerHandle,
    catalog_manager: crate::actors::catalog_manager::CatalogManagerHandle,
}

struct TestRigToolHandles {
    command: crate::actors::command::handle::CommandHandle,
    scanner: crate::actors::file_scanner::FileScannerHandle,
    guided_plan: crate::actors::guided_plan::GuidedPlanHandle,
    ask: crate::actors::ask::AskHandle,
    logger: crate::actors::LoggerHandle,
}

struct TestRigJoins {
    _token_tracker_join: tokio::task::JoinHandle<()>,
    _scanner_join: tokio::task::JoinHandle<()>,
    _logger_join: tokio::task::JoinHandle<()>,
    _catalog_manager_join: tokio::task::JoinHandle<()>,
}

struct TestRigTempDirs {
    _persistence_dir: tempfile::TempDir,
    _ask_dir: tempfile::TempDir,
}

struct TestRig {
    provider: Arc<RecordingChatProvider>,
    core: TestRigCoreHandles,
    tools: TestRigToolHandles,
    _joins: TestRigJoins,
    _temp_dirs: TestRigTempDirs,
}

impl TestRig {
    async fn new() -> Self {
        let provider = Arc::new(RecordingChatProvider::new());
        let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
        let dir = tempfile::tempdir().expect("tempdir");
        let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
        let (_token_tracker_join, token_tracker) = token_tracker::token_tracker_actor::spawn();
        let command = crate::actors::command::command_actor::build(&[]);
        let (scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
        let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
        let (ask, ask_dir) = crate::tests::helpers::fake_ask::make_ask_handle().await;
        let (logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
        let (catalog_manager_join, catalog_manager) =
            crate::tests::helpers::fake_catalog_manager::fake_catalog_manager_handle();
        Self {
            provider,
            core: TestRigCoreHandles {
                session,
                persistence,
                token_tracker,
                catalog_manager,
            },
            tools: TestRigToolHandles {
                command,
                scanner,
                guided_plan,
                ask,
                logger,
            },
            _joins: TestRigJoins {
                _token_tracker_join,
                _scanner_join: scanner_join,
                _logger_join: logger_join,
                _catalog_manager_join: catalog_manager_join,
            },
            _temp_dirs: TestRigTempDirs {
                _persistence_dir: dir,
                _ask_dir: ask_dir,
            },
        }
    }

    fn providers(&self) -> crate::actors::tui::tui_actor::TuiServiceHandles {
        crate::actors::tui::tui_actor::TuiServiceHandles::builder()
            .agent(self.provider.clone())
            .session(self.core.session.clone())
            .tools(
                crate::actors::tui::tui_actor::TuiServiceTools::builder()
                    .command(self.tools.command.clone())
                    .file_scanner(self.tools.scanner.clone())
                    .guided_plan(self.tools.guided_plan.clone())
                    .ask(self.tools.ask.clone())
                    .logger(self.tools.logger.clone())
                    .build(),
            )
            .orchestrator(crate::tests::helpers::fake_orchestrator::fake_orchestrator_handle())
            .catalog_manager(self.core.catalog_manager.clone())
            .build()
    }

    fn startup(
        &self,
        session_summaries: Vec<SessionSummary>,
    ) -> crate::actors::tui::tui_actor::TuiStartupData {
        crate::actors::tui::tui_actor::TuiStartupData::builder()
            .session_summaries(session_summaries)
            .persistence(self.core.persistence.clone())
            .token_tracker(self.core.token_tracker.clone())
            .config(test_config())
            .renderer(noop_renderer)
            .build()
    }
}

// ── build_initial_mode ───────────────────────────────────────────────────────

/// Verifies that an empty summary list produces `AppScreen::Conversation` so
/// the TUI opens directly in chat mode when no prior sessions exist.
#[test]
fn build_initial_mode_returns_conversation_for_empty_summaries() {
    let mode = super::build_initial_mode(vec![]);
    assert!(
        matches!(mode, AppScreen::Conversation),
        "empty summaries must produce Conversation startup mode"
    );
}

/// Verifies that a non-empty summary list produces `AppScreen::SessionSelector`
/// so the picker screen is shown at startup when sessions are available.
#[test]
fn build_initial_mode_returns_picker_for_non_empty_summaries() {
    let mode = super::build_initial_mode(vec![make_summary("s1")]);
    assert!(
        matches!(mode, AppScreen::SessionSelector(_)),
        "non-empty summaries must produce SessionSelector startup mode"
    );
}

/// Verifies that every summary in the input is present in the picker session
/// list; no sessions are dropped or added during the mapping.
#[test]
fn build_initial_mode_picker_session_count_equals_input_count() {
    let mode = super::build_initial_mode(vec![make_summary("a"), make_summary("b")]);
    let AppScreen::SessionSelector(PickerState { sessions, .. }) = mode else {
        panic!("expected SessionSelector");
    };
    assert_eq!(
        sessions.len(),
        2,
        "picker must contain exactly as many sessions as the input summary list"
    );
}

// ── into_picker_session ──────────────────────────────────────────────────────

/// Verifies that `into_picker_session` maps all identity fields from the
/// persistence `SessionSummary` into the corresponding `PickerSessionSummary`
/// fields, including message_count and preview text.
#[test]
fn into_picker_session_maps_all_fields_from_session_summary() {
    let summary = SessionSummary::builder()
        .identity(
            SessionIdentity::builder()
                .id(SessionId::new("abc-123"))
                .created_at(TimestampMs::new(100))
                .last_updated_at(TimestampMs::new(200))
                .endpoint_name(EndpointName::new("claude"))
                .build(),
        )
        .message_count(Count::new(7))
        .preview(OutputText::new("first user message"))
        .build();

    let result = super::into_picker_session(summary);

    assert_eq!(
        result.identity.id.as_str(),
        "abc-123",
        "session id must be preserved"
    );
    assert_eq!(
        result.identity.created_at,
        TimestampMs::new(100),
        "created_at must be preserved"
    );
    assert_eq!(
        result.identity.last_updated_at,
        TimestampMs::new(200),
        "last_updated_at must be preserved"
    );
    assert_eq!(
        result.identity.endpoint_name.as_str(),
        "claude",
        "endpoint_name must be preserved"
    );
    assert_eq!(
        result.message_count,
        Count::new(7),
        "message_count must be preserved"
    );
    assert_eq!(
        result.preview.as_str(),
        "first user message",
        "preview text must be preserved"
    );
}

/// Verifies that `build_initial_state` starts in conversation mode and calls
/// `replace_session(None)` when there are no saved sessions to pick from.
#[tokio::test]
async fn build_initial_state_empty_startup_replaces_session_with_none() {
    let rig = TestRig::new().await;
    let providers = rig.providers();
    let startup = rig.startup(vec![]);

    let state = super::build_initial_state(&providers, &startup);

    assert!(
        matches!(state.interaction.screen, AppScreen::Conversation),
        "empty startup summaries must open directly in Conversation mode"
    );
    assert_eq!(
        rig.provider.take_replace_calls(),
        vec![None],
        "conversation startup must reset the provider session with replace_session(None)"
    );
}

/// Verifies that `build_initial_state` starts in picker mode, maps the startup
/// summary fields into the picker rows, and does not reset the provider session.
#[tokio::test]
async fn build_initial_state_non_empty_startup_opens_picker_with_mapped_summary() {
    let rig = TestRig::new().await;
    let providers = rig.providers();
    let startup = rig.startup(vec![make_summary("picker-1")]);

    let state = super::build_initial_state(&providers, &startup);

    let AppScreen::SessionSelector(PickerState { sessions, selected }) = state.interaction.screen
    else {
        panic!("expected SessionSelector startup screen");
    };
    assert_eq!(
        selected,
        Count::new(0),
        "picker startup must select the first session row"
    );
    assert_eq!(
        sessions.len(),
        1,
        "picker startup must expose the saved session"
    );
    assert_eq!(sessions[0].identity.id.as_str(), "picker-1");
    assert_eq!(sessions[0].identity.endpoint_name.as_str(), "ep");
    assert_eq!(sessions[0].message_count, Count::new(3));
    assert_eq!(sessions[0].preview.as_str(), "hello");
    assert!(
        rig.provider.take_replace_calls().is_empty(),
        "picker startup must not call replace_session(None) before the user picks a session"
    );
}
