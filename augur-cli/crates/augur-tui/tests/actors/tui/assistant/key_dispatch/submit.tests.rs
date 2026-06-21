use crate::domain::newtypes::{NumericNewtype, ScrollOffset};
use crate::domain::string_newtypes::{EndpointName, ModelId, PromptText, StringNewtype};
use crate::domain::thinking_mode::ReasoningEffort;
use crate::domain::traits::ChatProvider;
use crate::domain::tui_state::{
    AppScreen, AppState, ConversationMode, OutputSelection, SelectionPoint,
};
use crate::domain::types::AgentOutput;
use crate::persistence::types::MessageRecord;
use crate::tests::helpers::fake_ask;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// `(model_id_str, Option<effort_str>)` pairs recorded by `set_model_with_options`.
type ModelWithOptionsCall = (String, Option<String>);

struct RecordingChatProvider {
    state: RecordingChatProviderState,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
}

struct RecordingChatProviderState {
    submit_prompts: Arc<Mutex<Vec<String>>>,
    compact_calls: Arc<Mutex<usize>>,
    interrupt_calls: Arc<Mutex<usize>>,
    model_calls: Arc<Mutex<Vec<String>>>,
    /// Tracks `set_model_with_options` calls as `(model_id, Option<effort_str>)`.
    model_with_options_calls: Arc<Mutex<Vec<ModelWithOptionsCall>>>,
}

impl RecordingChatProvider {
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(4);
        Self {
            state: RecordingChatProviderState {
                submit_prompts: Arc::new(Mutex::new(Vec::new())),
                compact_calls: Arc::new(Mutex::new(0)),
                interrupt_calls: Arc::new(Mutex::new(0)),
                model_calls: Arc::new(Mutex::new(Vec::new())),
                model_with_options_calls: Arc::new(Mutex::new(Vec::new())),
            },
            output_tx,
        }
    }

    fn submit_prompts(&self) -> Vec<String> {
        self.state.submit_prompts.lock().unwrap().clone()
    }

    fn compact_count(&self) -> usize {
        *self.state.compact_calls.lock().unwrap()
    }

    fn interrupt_count(&self) -> usize {
        *self.state.interrupt_calls.lock().unwrap()
    }

    fn model_calls(&self) -> Vec<String> {
        self.state.model_calls.lock().unwrap().clone()
    }

    fn model_with_options_calls(&self) -> Vec<ModelWithOptionsCall> {
        self.state.model_with_options_calls.lock().unwrap().clone()
    }
}

impl ChatProvider for RecordingChatProvider {
    fn submit(&self, prompt: PromptText, _endpoint: Option<EndpointName>) {
        self.state
            .submit_prompts
            .lock()
            .unwrap()
            .push(prompt.to_string());
    }

    fn interrupt(&self) {
        *self.state.interrupt_calls.lock().unwrap() += 1;
    }

    fn shutdown(&self) {}

    fn restore(&self, _records: Vec<MessageRecord>) {}

    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    fn compact(&self) {
        *self.state.compact_calls.lock().unwrap() += 1;
    }

    fn set_model(&self, model_id: ModelId) {
        self.state
            .model_calls
            .lock()
            .unwrap()
            .push(model_id.to_string());
    }

    fn set_model_with_options(&self, model_id: ModelId, reasoning_effort: Option<ReasoningEffort>) {
        let effort_str = reasoning_effort.map(|e| e.as_ref().to_owned());
        self.state
            .model_with_options_calls
            .lock()
            .unwrap()
            .push((model_id.to_string(), effort_str));
    }
}

struct SubmitHarnessCoreHandles {
    session: crate::actors::SessionHandle,
    persistence: crate::persistence::handle::PersistenceHandle,
}

struct SubmitHarnessToolHandles {
    command: crate::actors::command::handle::CommandHandle,
    scanner: crate::actors::file_scanner::FileScannerHandle,
    guided_plan: crate::actors::guided_plan::GuidedPlanHandle,
    ask: crate::actors::ask::AskHandle,
    logger: crate::actors::LoggerHandle,
}

struct SubmitHarnessResources {
    _persistence_dir: tempfile::TempDir,
    _scanner_join: tokio::task::JoinHandle<()>,
    _ask_dir: tempfile::TempDir,
    _logger_join: tokio::task::JoinHandle<()>,
}

struct SubmitHarness {
    provider: RecordingChatProvider,
    core: SubmitHarnessCoreHandles,
    tools: SubmitHarnessToolHandles,
    _resources: SubmitHarnessResources,
}

impl SubmitHarness {
    async fn new(
        provider: RecordingChatProvider,
        guided_plan: crate::actors::guided_plan::GuidedPlanHandle,
    ) -> Self {
        let command = crate::actors::command::command_actor::build(&[]);
        let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
        let persistence_dir = tempfile::tempdir().expect("tempdir");
        let persistence =
            crate::persistence::handle::PersistenceHandle::new(persistence_dir.path().to_owned());
        let (scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
        let (ask, ask_dir) = fake_ask::make_ask_handle().await;
        let (logger_join, logger) = crate::tests::helpers::fake_logger::fake_logger_handle();
        Self {
            provider,
            core: SubmitHarnessCoreHandles {
                session,
                persistence,
            },
            tools: SubmitHarnessToolHandles {
                command,
                scanner,
                guided_plan,
                ask,
                logger,
            },
            _resources: SubmitHarnessResources {
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
        self.handles_with_catalog_manager(catalog_manager)
    }

    fn handles_with_catalog_manager(
        &self,
        catalog_manager: crate::actors::catalog_manager::CatalogManagerHandle,
    ) -> crate::actors::tui::tui_actor::TuiHandles<'_> {
        crate::actors::tui::tui_actor::TuiHandles {
            agent: &self.provider,
            session: &self.core.session,
            persistence: &self.core.persistence,
            tools: crate::actors::tui::tui_actor::TuiToolHandles {
                command: &self.tools.command,
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

fn output_text(state: &AppState) -> String {
    state
        .output
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn make_guided_plan_command_handle() -> (
    crate::actors::guided_plan::GuidedPlanHandle,
    tokio::sync::mpsc::Receiver<crate::actors::guided_plan::commands::GuidedPlanCmd>,
) {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(4);
    let (event_tx, _) =
        tokio::sync::broadcast::channel::<crate::domain::guided_plan::GuidedPlanEvent>(4);
    (
        crate::actors::guided_plan::GuidedPlanHandle { cmd_tx, event_tx },
        cmd_rx,
    )
}

fn write_guided_plan_file() -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("guided plan file");
    file.write_all(
        br#"---
guided: true
name: "Coverage Plan"
phases:
  - id: "phase-1"
    name: "First Phase"
---
# Coverage Plan
"#,
    )
    .expect("write guided plan file");
    file
}

fn write_invalid_guided_plan_file() -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("invalid guided plan file");
    file.write_all(b"# missing guided frontmatter\n")
        .expect("write invalid guided plan file");
    file
}

/// Verifies that `/compact` routes directly to `ChatProvider::compact` without submitting chat text.
#[tokio::test]
async fn handle_submit_compact_routes_to_provider_compact() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/compact".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/compact must not quit the TUI"
    );
    assert_eq!(harness.provider.compact_count(), 1);
    assert!(
        harness.provider.submit_prompts().is_empty(),
        "/compact must not fall through to normal submit"
    );
}

/// Verifies that `/clear` starts a fresh local session view and does not submit chat text.
#[tokio::test]
async fn handle_submit_clear_resets_session_view_without_chat_submit() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/clear".to_owned();
    state.status.token_totals.tokens_in = crate::domain::TokenCount::new(77);
    state.status.reset_usage_on_next_snapshot = false;

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/clear must not quit the TUI"
    );
    assert!(
        harness.provider.submit_prompts().is_empty(),
        "/clear must not fall through to normal chat submit"
    );
    assert_eq!(
        state.status.token_totals,
        crate::domain::types::ProjectTokenTotals::default(),
        "/clear must reset displayed token totals immediately"
    );
    assert!(
        state.status.reset_usage_on_next_snapshot,
        "/clear must schedule token baseline reset on next snapshot tick"
    );
    assert!(
        output_text(&state).contains("[system] new session started"),
        "/clear must show new-session confirmation"
    );
}

/// Verifies `/new-session` + `/clear` flow performs provider-aware OpenRouter reset routing.
///
/// Phase 4 requires `handle_new_session` to use provider-aware routing (via active
/// endpoint flow) so OpenRouter sessions are reset through provider orchestration.
#[test]
fn new_session_command_resets_openrouter_provider_session() {
    let source = include_str!("../../../../../src/actors/tui/assistant/key_dispatch/submit.rs");
    let start = source
        .find("fn handle_new_session")
        .expect("submit.rs must define handle_new_session");
    let tail = &source[start..];
    let end = tail
        .find("async fn handle_generate_catalog")
        .expect("handle_new_session block boundary must exist");
    let body = &tail[..end];
    assert!(
        body.contains("active_endpoint"),
        "handle_new_session must be provider-aware by consulting the active endpoint for OpenRouter session reset routing"
    );
}

/// Verifies settings persistence in submit flows routes through the session facade.
#[test]
fn submit_routes_user_settings_persistence_through_session_handle() {
    let source = include_str!("../../../../../src/actors/tui/assistant/key_dispatch/submit.rs");
    assert!(
        source.contains("handles.session.save_user_settings("),
        "submit handlers must persist settings through SessionHandle facade"
    );
    assert!(
        !source.contains("crate::config::user_settings::save_user_settings("),
        "submit handlers must not write user settings directly from the TUI layer"
    );
}

/// Verifies that `/stop` appends stop feedback and routes directly to `ChatProvider::interrupt`.
#[tokio::test]
async fn handle_submit_stop_routes_to_interrupt_with_feedback() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/stop".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/stop must not quit the TUI"
    );
    assert_eq!(harness.provider.interrupt_count(), 1);
    assert!(
        output_text(&state).contains("[system] stopping current execution..."),
        "/stop must render user-visible stop feedback"
    );
}

/// Verifies that `/commit` echoes the command, enters the committing state, and submits the commit prompt.
#[tokio::test]
async fn handle_submit_commit_routes_to_special_agent_prompt() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/commit".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/commit must not quit the TUI"
    );
    assert_eq!(
        harness.provider.submit_prompts(),
        vec!["create message and commit".to_owned()]
    );
    assert!(state.agent.thinking.is_active);
    assert_eq!(state.agent.thinking.label.as_str(), "Committing...");
    assert!(output_text(&state).contains("> /commit"));
}

/// Verifies that `/push` echoes the command, enters the pushing state, and submits the push prompt.
#[tokio::test]
async fn handle_submit_push_routes_to_special_agent_prompt() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/push".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/push must not quit the TUI"
    );
    assert_eq!(
        harness.provider.submit_prompts(),
        vec!["push commits to remote origin".to_owned()]
    );
    assert!(state.agent.thinking.is_active);
    assert_eq!(state.agent.thinking.label.as_str(), "Pushing...");
    assert!(output_text(&state).contains("> /push"));
}

/// Verifies that a plain user prompt is echoed into the main conversation feed
/// before dispatching to the chat provider.
#[tokio::test]
async fn handle_submit_plain_prompt_echoes_user_line_to_main_feed() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "hello from user".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "plain prompt submit must not quit the TUI"
    );
    assert_eq!(
        harness.provider.submit_prompts(),
        vec!["hello from user".to_owned()],
        "plain prompt must be forwarded to the provider"
    );
    assert!(
        output_text(&state).contains("> hello from user"),
        "plain prompt must be echoed as a user line in the main conversation feed"
    );
}

/// Regression: stale nonzero main-feed scroll must reset on plain submit so the
/// user line is immediately visible.
#[tokio::test]
async fn handle_submit_plain_prompt_resets_stale_scroll_before_user_line_append() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.push_output_newline();
    state.push_output_newline();
    state.output.scroll_offset.set(ScrollOffset::of(7));
    state.output.selection = Some(OutputSelection {
        anchor: SelectionPoint { col: 0, row: 0 },
        cursor: SelectionPoint { col: 1, row: 0 },
    });
    state.prompt.buffer = "plain prompt".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "plain prompt submit must not quit the TUI"
    );
    assert_eq!(
        state.output.scroll_offset.get().inner(),
        0,
        "plain prompt submit must re-anchor main feed to bottom"
    );
    assert!(
        state.output.selection.is_none(),
        "plain prompt submit must clear output selection for stable redraw visibility"
    );
    assert!(
        output_text(&state).contains("> plain prompt"),
        "plain prompt must still append a visible user line in main feed"
    );
}

/// Regression: hidden/stale Ask focus (Ask panel not visible) must not steal Enter.
///
/// When `input_focus == Ask` but `secondary_view != Some(Ask)` and `ask_panel == None`,
/// submitting plain text must route through the main submit path.
#[tokio::test]
async fn handle_submit_hidden_ask_focus_still_routes_plain_prompt_to_main_feed() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "hello from main".to_owned();
    state.interaction.panel.input_focus = crate::domain::tui_state::InputFocus::Ask;
    state.interaction.panel.secondary_view = None;
    state.interaction.panel.ask_panel = None;

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "plain prompt submit must not quit the TUI"
    );
    assert_eq!(
        harness.provider.submit_prompts(),
        vec!["hello from main".to_owned()],
        "hidden Ask focus must not reroute plain prompt away from main submit"
    );
    assert!(
        output_text(&state).contains("> hello from main"),
        "plain prompt must still be echoed in main conversation feed"
    );
}

/// Verifies that `/switch <endpoint>` routes through the session handle and renders the endpoint switch confirmation.
#[tokio::test]
async fn handle_submit_switch_routes_to_session_endpoint_change() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog =
        vec![crate::domain::tui_state::EndpointModelCatalog::builder()
            .endpoint_name(EndpointName::new("alt-endpoint"))
            .models(vec![crate::domain::types::ModelOption::builder()
                .id(ModelId::new("gpt-4.1"))
                .display_name("gpt-4.1 (openrouter)".into())
                .build()])
            .default_display("gpt-4.1 (high)".into())
            .supports_auto(false)
            .build()];
    state.prompt.models.available = vec![crate::domain::types::ModelOption::builder()
        .id(ModelId::new("old-copilot-model"))
        .display_name("old-copilot-model".into())
        .build()];
    state.prompt.buffer = "/switch alt-endpoint".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/switch must not quit the TUI"
    );
    assert_eq!(
        harness.core.session.active_endpoint().as_str(),
        "alt-endpoint"
    );
    assert_eq!(state.prompt.models.available.len(), 1);
    assert_eq!(state.prompt.models.available[0].id.as_str(), "gpt-4.1");
    assert_eq!(state.status.model_display.as_str(), "gpt-4.1 (high)");
    assert!(
        output_text(&state).contains("[system] switched to endpoint: alt-endpoint"),
        "/switch must render the endpoint confirmation"
    );
}

#[tokio::test]
async fn handle_submit_switch_reports_failure_when_session_queue_unavailable() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/switch alt-endpoint".to_owned();
    harness.core.session.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/switch failure must not quit the TUI"
    );
    assert_eq!(
        harness.core.session.active_endpoint().as_str(),
        "ep",
        "endpoint must remain unchanged when enqueue fails"
    );
    assert!(
        output_text(&state).contains("[system] failed to switch endpoint: alt-endpoint"),
        "failed enqueue must render an explicit failure message"
    );
}

#[tokio::test]
async fn handle_submit_switch_to_auto_provider_resets_model_to_auto() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog =
        vec![crate::domain::tui_state::EndpointModelCatalog::builder()
            .endpoint_name(EndpointName::new("copilot"))
            .models(vec![])
            .default_display("copilot".into())
            .supports_auto(true)
            .build()];
    state.prompt.buffer = "/switch copilot".to_owned();

    let _ = super::handle_submit(&mut state, &harness.handles()).await;

    assert_eq!(harness.provider.model_calls(), vec![String::new()]);
    assert_eq!(
        state.prompt.models.active_id.as_ref().map(|id| id.as_str()),
        Some("")
    );
}

#[tokio::test]
async fn handle_submit_generate_catalog_refreshes_models_from_provider_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("openrouter.yaml"),
        r#"
provider: openrouter
models:
  - id: anthropic/claude-sonnet-4-5
    display_name: Claude Sonnet 4.5
    cost_input_per_mtok: 3.0
    cost_output_per_mtok: 15.0
"#,
    )
    .expect("write provider file");

    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    assert!(harness
        .core
        .session
        .set_endpoint(EndpointName::new("alt-endpoint"))
        .await
        .is_ok());
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let mut state = AppState::new(EndpointName::new("alt-endpoint"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog =
        vec![crate::domain::tui_state::EndpointModelCatalog::builder()
            .endpoint_name(EndpointName::new("alt-endpoint"))
            .models(vec![crate::domain::types::ModelOption::builder()
                .id(ModelId::new("old/model"))
                .display_name("old/model".into())
                .build()])
            .default_display("old/model (high)".into())
            .supports_auto(false)
            .build()];
    let config = crate::config::types::AppConfig {
        endpoints: vec![crate::config::types::EndpointConfig {
            name: EndpointName::new("alt-endpoint"),
            provider: crate::config::types::Provider::OpenRouter,
            base_url: crate::domain::string_newtypes::EndpointUrl::new(
                "https://openrouter.ai/api/v1",
            ),
            model: crate::domain::string_newtypes::ModelName::new("anthropic/claude-sonnet-4-5"),
            credentials: crate::config::types::EndpointCredentials::default(),
        }],
        default_endpoint: EndpointName::new("alt-endpoint"),
        agent: crate::config::types::AgentConfig {
            system_prompt: "sys".into(),
            max_tokens: crate::domain::newtypes::TokenCount::new(1024),
            temperature: crate::domain::newtypes::Temperature::new(0.7),
            allowed_dirs: vec![],
        },
        copilot: crate::config::types::CopilotConfig::default(),
        persistence: crate::config::types::PersistenceConfig {
            log_dir: crate::domain::string_newtypes::FilePath::new("./logs"),
            sessions_dir: None,
        },
            program_settings: Default::default(),
            user_settings: Default::default(),
    };
    super::refresh_endpoint_catalog_from_provider_dir(
        &mut state,
        &harness.handles(),
        super::RefreshEndpointCatalogArgs {
            config: &config,
            provider_dir: tmp.path(),
        },
    );

    assert!(
        state
            .prompt
            .models
            .available
            .iter()
            .any(|m| m.id.as_str() == "anthropic/claude-sonnet-4-5"),
        "in-memory model list must refresh from rewritten provider file"
    );
}

#[tokio::test]
async fn handle_submit_generate_catalog_command_writes_and_refreshes_model_list() {
    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }
    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.previous {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    let temp_root = tempfile::tempdir().expect("temp root");
    let provider_dir = temp_root.path().join("configs/providers");
    std::fs::create_dir_all(&provider_dir).expect("create provider dir");
    let config_dir = temp_root.path().join(".config/augur-cli");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("config.yaml");
    std::fs::write(
        &config_path,
        r#"
endpoints:
  - name: ep
    provider: OpenRouter
    base_url: "https://openrouter.ai/api/v1"
    model: "anthropic/claude-sonnet-4-5"
    api_key_env: OPENROUTER_API_KEY
default_endpoint: ep
agent:
  system_prompt: "sys"
  max_tokens: 1024
  temperature: 0.7
  allowed_dirs: ["./"]
copilot_chat:
  enabled: false
log_dir: "./logs"
"#,
    )
    .expect("write config");
    let _provider_dir_env = EnvVarGuard::set(
        "AUGUR_CLI_PROVIDER_CATALOG_DIR",
        provider_dir.to_string_lossy().as_ref(),
    );
    let _config_env = EnvVarGuard::set(
        "AUGUR_CLI_CONFIG_PATH",
        config_path.to_string_lossy().as_ref(),
    );

    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(1);
    let catalog_manager = crate::actors::catalog_manager::handle::CatalogManagerHandle::new(cmd_tx);
    tokio::spawn(async move {
        if let Some(
            crate::actors::catalog_manager::handle::CatalogManagerCommand::GenerateCatalog {
                tx,
                ..
            },
        ) = cmd_rx.recv().await
        {
            std::fs::write(
                provider_dir.join("openrouter.yaml"),
                r#"
provider: openrouter
models:
  - id: openrouter/new-model
    display_name: New OpenRouter Model
    cost_input_per_mtok: 1.0
    cost_output_per_mtok: 2.0
"#,
            )
            .expect("write provider file");
            let _ = tx.send(Ok(crate::domain::string_newtypes::OutputText::from(
                "generated",
            )));
        }
    });

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![crate::domain::types::ModelOption::builder()
        .id(ModelId::new("old/model"))
        .display_name("old/model".into())
        .build()];
    state.prompt.buffer = "/generate-catalog --provider openrouter".to_owned();

    let should_quit = super::handle_submit(
        &mut state,
        &harness.handles_with_catalog_manager(catalog_manager),
    )
    .await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/generate-catalog must not quit the TUI"
    );
    assert!(
        state
            .prompt
            .models
            .available
            .iter()
            .any(|m| m.id.as_str() == "openrouter/new-model"),
        "command path must refresh in-memory model list from written provider file"
    );
}

/// Verifies that `/model <id>` opens the thinking mode picker instead of immediately calling set_model.
///
/// After this change, submitting `/model gpt-5` stores the pending model id in
/// the thinking mode completion state so the user can choose a reasoning effort
/// before the model is applied.
#[tokio::test]
async fn handle_submit_model_id_opens_thinking_mode_picker() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![crate::domain::types::ModelOption::builder()
        .id(ModelId::new("gpt-5"))
        .display_name("gpt-5".into())
        .build()];
    state.prompt.buffer = "/model gpt-5".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/model <id> must not quit the TUI"
    );
    assert!(
        harness.provider.model_calls().is_empty(),
        "/model <id> must not immediately call set_model; thinking mode picker must open first"
    );
    assert!(
        harness.provider.model_with_options_calls().is_empty(),
        "/model <id> must not immediately call set_model_with_options"
    );
    assert_eq!(
        state
            .prompt
            .completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("gpt-5"),
        "thinking mode picker must hold the pending model id"
    );
}

/// Verifies that confirming the thinking mode picker calls set_model_with_options.
///
/// After the thinking mode picker opens, pressing Enter with a selected reasoning
/// effort should call `set_model_with_options(model_id, Some(effort))` and clear
/// the pending state.
#[tokio::test]
async fn handle_submit_thinking_mode_confirm_calls_set_model_with_options() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    // Simulate the state after `/model gpt-5` was submitted (thinking mode opened).
    state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id = Some(ModelId::new("gpt-5"));
    // Select "high" (index 1 in the default options order: auto, high, medium, low, none)
    state.prompt.completions.model_picker.thinking_mode.selected = Some(1);

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "thinking mode confirm must not quit the TUI"
    );
    let calls = harness.provider.model_with_options_calls();
    assert_eq!(calls.len(), 1, "set_model_with_options must be called once");
    assert_eq!(calls[0].0, "gpt-5", "model id must match the pending model");
    assert_eq!(
        calls[0].1.as_deref(),
        Some("high"),
        "reasoning effort must match the selected option"
    );
    assert!(
        state
            .prompt
            .completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .is_none(),
        "pending_model_id must be cleared after confirmation"
    );
}

/// Verifies that thinking mode confirm with None selection defaults to Auto.
#[tokio::test]
async fn handle_submit_thinking_mode_confirm_defaults_to_auto_when_no_selection() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id = Some(ModelId::new("gpt-5"));
    state.prompt.completions.model_picker.thinking_mode.selected = None; // no selection → default to Auto

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(matches!(should_quit, std::ops::ControlFlow::Continue(())));
    let calls = harness.provider.model_with_options_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].1.as_deref(),
        Some("auto"),
        "no selection must default to auto reasoning effort"
    );
}

/// Verifies that thinking mode is cleared when completions are cleared (Escape path).
#[test]
fn thinking_mode_cleared_by_clear_all_completions() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id = Some(ModelId::new("gpt-5"));
    state.prompt.completions.model_picker.thinking_mode.selected = Some(0);

    super::clear_all_completions(&mut state);

    assert!(
        state
            .prompt
            .completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .is_none(),
        "clear_all_completions must clear thinking mode pending_model_id"
    );
    assert!(
        state
            .prompt
            .completions
            .model_picker
            .thinking_mode
            .selected
            .is_none(),
        "clear_all_completions must clear thinking mode selection"
    );
}

/// Verifies that thinking mode is treated as an open completion (not empty).
#[test]
fn thinking_mode_open_means_completions_not_empty() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id = Some(ModelId::new("gpt-5"));

    assert!(
        !state.prompt.completions.is_empty().0,
        "when thinking mode is open, completions must report non-empty"
    );
}

/// Verifies that bare `/model` routes to auto-model selection and updates the visible model label.
#[tokio::test]
async fn handle_submit_model_without_id_routes_to_auto_model() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog =
        vec![crate::domain::tui_state::EndpointModelCatalog::builder()
            .endpoint_name(EndpointName::new("ep"))
            .models(vec![])
            .default_display("copilot".into())
            .supports_auto(true)
            .build()];
    state.status.model_display = "manual".into();
    state.prompt.buffer = "/model".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/model must not quit the TUI"
    );
    assert_eq!(harness.provider.model_calls(), vec![String::new()]);
    assert_eq!(state.status.model_display.as_str(), "auto");
    assert!(
        output_text(&state).contains("[system] model: auto"),
        "bare /model must render the auto-model confirmation"
    );
}

#[tokio::test]
async fn handle_submit_model_without_id_reports_unsupported_for_non_auto_endpoint() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog =
        vec![crate::domain::tui_state::EndpointModelCatalog::builder()
            .endpoint_name(EndpointName::new("ep"))
            .models(vec![])
            .default_display("manual-model".into())
            .supports_auto(false)
            .build()];
    state.prompt.buffer = "/model".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(matches!(should_quit, std::ops::ControlFlow::Continue(())));
    assert!(
        harness.provider.model_calls().is_empty(),
        "non-auto endpoint must not trigger set_model(\"\")"
    );
    assert!(
        output_text(&state).contains("auto model selection is not supported"),
        "bare /model must report unsupported for non-auto endpoints"
    );
}

#[tokio::test]
async fn handle_submit_model_with_unknown_id_rejected_for_non_auto_endpoint() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.endpoint_catalog =
        vec![crate::domain::tui_state::EndpointModelCatalog::builder()
            .endpoint_name(EndpointName::new("ep"))
            .models(vec![crate::domain::types::ModelOption::builder()
                .id(ModelId::new("known/model"))
                .display_name("known/model".into())
                .build()])
            .default_display("known/model".into())
            .supports_auto(false)
            .build()];
    state.prompt.models.available = vec![crate::domain::types::ModelOption::builder()
        .id(ModelId::new("known/model"))
        .display_name("known/model".into())
        .build()];
    state.prompt.buffer = "/model unknown/model".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(matches!(should_quit, std::ops::ControlFlow::Continue(())));
    assert!(
        state
            .prompt
            .completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .is_none(),
        "unknown model must not open thinking mode for non-auto endpoints"
    );
    assert!(
        output_text(&state).contains("is not available for endpoint"),
        "unknown model must render rejection message"
    );
}

/// Verifies that `/run-plan <path>` enters guided-plan mode and forwards the parsed config to the guided-plan handle.
#[tokio::test]
async fn handle_submit_run_plan_enters_guided_plan_mode_and_starts_actor() {
    let (guided_plan, mut cmd_rx) = make_guided_plan_command_handle();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let plan_file = write_guided_plan_file();
    state.prompt.buffer = format!("/run-plan {}", plan_file.path().display());

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/run-plan must not quit the TUI"
    );
    match &state.interaction.mode {
        ConversationMode::GuidedPlan(ui) => {
            assert_eq!(ui.plan_name.as_str(), "Coverage Plan");
            assert_eq!(ui.phases.len(), 1);
            assert_eq!(ui.phases[0].0, "First Phase");
        }
        _ => panic!("/run-plan must enter ConversationMode::GuidedPlan"),
    }
    match tokio::time::timeout(std::time::Duration::from_millis(50), cmd_rx.recv()).await {
        Ok(Some(crate::actors::guided_plan::commands::GuidedPlanCmd::Start {
            config,
            plan_path,
        })) => {
            assert_eq!(config.name.as_str(), "Coverage Plan");
            assert_eq!(
                plan_path.as_str(),
                plan_file.path().to_str().expect("utf-8 path")
            );
        }
        other => panic!("expected GuidedPlanCmd::Start, got {other:?}"),
    }
    assert!(
        output_text(&state).contains("[system] guided plan started:"),
        "/run-plan must render the guided-plan start confirmation"
    );
}

/// Verifies that `/run-plan <path>` surfaces loader failures without entering
/// guided-plan mode or sending a start command to the guided-plan actor.
#[tokio::test]
async fn handle_submit_run_plan_load_failure_keeps_chat_mode_and_skips_start_command() {
    let (guided_plan, mut cmd_rx) = make_guided_plan_command_handle();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let invalid_plan_file = write_invalid_guided_plan_file();
    state.prompt.buffer = format!("/run-plan {}", invalid_plan_file.path().display());

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/run-plan must not quit the TUI when loading fails"
    );
    assert!(
        !matches!(state.interaction.mode, ConversationMode::GuidedPlan(_)),
        "loader failure must leave the TUI out of guided-plan mode"
    );
    assert!(
        output_text(&state).contains("[error] /run-plan:"),
        "loader failure must render a user-visible error line"
    );
    assert!(
        output_text(&state).contains("guided: true"),
        "loader failure must surface the loader reason"
    );
    match tokio::time::timeout(std::time::Duration::from_millis(50), cmd_rx.recv()).await {
        Err(_) => {}
        other => panic!("expected no GuidedPlanCmd::Start on loader failure, got {other:?}"),
    }
}

/// Verifies that an unknown slash command stays on the submit path and produces unknown-command feedback.
#[tokio::test]
async fn handle_submit_unknown_command_renders_unknown_command_feedback() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/not-a-real-command".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "unknown commands must not quit the TUI"
    );
    assert!(
        output_text(&state).contains("[system] unknown command: /not-a-real-command"),
        "unknown slash commands must render a user-visible error"
    );
}

/// Verifies that `parse_slug_flag` extracts the slug and returns the remaining text when
/// `--slug <value>` appears at the beginning.
#[test]
fn start_pipeline_with_slug_flag_extracts_slug() {
    let (slug, remainder) = super::parse_slug_flag("--slug my-feature context text");
    assert_eq!(slug, Some("my-feature".to_owned()));
    assert_eq!(remainder, "context text");
}

/// Verifies that `parse_slug_flag` returns `None` for the slug when no `--slug` flag is present.
#[test]
fn start_pipeline_without_slug_flag_uses_none() {
    let (slug, remainder) = super::parse_slug_flag("context text");
    assert_eq!(slug, None);
    assert_eq!(remainder, "context text");
}

/// Verifies that `parse_slug_flag` extracts the slug when `--slug <value>` appears at the end.
#[test]
fn start_pipeline_slug_flag_at_end() {
    let (slug, remainder) = super::parse_slug_flag("context text --slug my-feature");
    assert_eq!(slug, Some("my-feature".to_owned()));
    assert_eq!(remainder, "context text");
}

/// Verifies that `/compact` echoes the command to the conversation panel.
///
/// Even though `/compact` triggers no agent response, the user's slash command
/// must appear as a user-visible entry in the conversation panel.
#[tokio::test]
async fn handle_submit_compact_echoes_command_to_conversation_panel() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/compact".to_owned();

    let _ = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        output_text(&state).contains("> /compact"),
        "/compact must echo the command to the conversation panel"
    );
}

/// Verifies that `/stop` echoes the command to the conversation panel BEFORE the
/// `[system] stopping current execution...` status line.
#[tokio::test]
async fn handle_submit_stop_echoes_command_before_system_status_line() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/stop".to_owned();

    let _ = super::handle_submit(&mut state, &harness.handles()).await;

    let text = output_text(&state);

    let cmd_pos = text
        .find("> /stop")
        .expect("raw command must appear in the conversation panel as a user message");
    let sys_pos = text
        .find("[system] stopping current execution...")
        .expect("system status line must appear in the conversation panel");

    assert!(
        cmd_pos < sys_pos,
        "user command echo must appear BEFORE the system status line"
    );
}

/// the `[system] starting pipeline...` status line.
///
/// The conversation panel must read:
/// ```
/// > /run-pipeline --slug my-feature build a slug derivation pipeline
/// [system] starting pipeline (slug: my-feature)...
/// ```
#[tokio::test]
async fn start_pipeline_echoes_command_before_system_status_line() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer =
        "/run-pipeline --slug my-feature build a slug derivation pipeline".to_owned();

    let should_quit = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/run-pipeline must not quit the TUI"
    );

    let text = output_text(&state);

    let cmd_pos = text
        .find("> /run-pipeline --slug my-feature build a slug derivation pipeline")
        .expect("raw command must appear in the conversation panel as a user message");
    let sys_pos = text
        .find("[system] starting pipeline (slug: my-feature)...")
        .expect("system status line must appear in the conversation panel");

    assert!(
        cmd_pos < sys_pos,
        "user command echo must appear BEFORE the system status line"
    );
}

// ── /ping submit path integration tests (BEH-009, BEH-010, BEH-011) ──────────

#[tokio::test]
async fn test_handle_submit_ping_buffer_writes_pong_line_to_output_panel() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/ping".to_owned();

    let _ = super::handle_submit(&mut state, &harness.handles()).await;

    let text = output_text(&state);
    assert!(
        text.contains("> /ping"),
        "output panel must contain the echo line \"> /ping\", got:\n{text}"
    );
    assert!(
        text.contains("[system] pong"),
        "output panel must contain \"[system] pong\", got:\n{text}"
    );
    let echo_pos = text.find("> /ping").expect("echo line must be present");
    let pong_pos = text
        .find("[system] pong")
        .expect("[system] pong must be present");
    assert!(
        echo_pos < pong_pos,
        "echo line must appear before \"[system] pong\" in the output panel"
    );
    assert!(
        harness.provider.submit_prompts().is_empty(),
        "/ping must not submit any prompt to the agent"
    );
}

#[tokio::test]
async fn test_handle_submit_ping_buffer_does_not_activate_agent_thinking() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/ping".to_owned();
    state.agent.thinking.is_active = false;

    let result = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        matches!(result, std::ops::ControlFlow::Continue(())),
        "/ping must return ControlFlow::Continue(()), not Break"
    );
    assert!(
        !state.agent.thinking.is_active,
        "agent thinking must remain inactive after /ping"
    );
    assert!(
        harness.provider.submit_prompts().is_empty(),
        "/ping must not submit a prompt to the agent"
    );
}

#[tokio::test]
async fn test_handle_submit_ping_buffer_clears_prompt_buffer() {
    let guided_plan = crate::actors::guided_plan::guided_plan_actor::spawn();
    let harness = SubmitHarness::new(RecordingChatProvider::new(), guided_plan).await;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/ping".to_owned();

    let _ = super::handle_submit(&mut state, &harness.handles()).await;

    assert!(
        state.prompt.buffer.is_empty(),
        "prompt buffer must be empty after handle_submit"
    );
}
