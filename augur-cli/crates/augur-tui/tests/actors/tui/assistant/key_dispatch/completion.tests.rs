use crate::domain::string_newtypes::{EndpointName, FilePath, ModelId, ModelLabel, StringNewtype};
use crate::domain::tui_state::{AppScreen, AppState};
use crate::domain::types::{CommandDef, FileCompletion, ModelOption};

fn conversation_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

fn model_option(id: &str) -> ModelOption {
    ModelOption::builder()
        .id(ModelId::new(id))
        .display_name(ModelLabel::new(id))
        .build()
}

static FAKE_CMD: CommandDef = CommandDef {
    name: "quit",
    usage: "/quit",
    description: "Quit the TUI",
};

fn fake_file(path: &str) -> FileCompletion {
    FileCompletion {
        path: FilePath::new(path),
        display_name: path.rsplit('/').next().unwrap_or(path).to_owned().into(),
    }
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

// ── close_completions_if_open ────────────────────────────────────────────────

/// Verifies that `close_completions_if_open` returns `None` when all completion
/// lists are empty, so the caller can skip an unnecessary re-render.
#[test]
fn close_completions_if_open_returns_none_when_all_completions_are_empty() {
    let mut state = conversation_state();
    let result = super::close_completions_if_open(&mut state);
    assert!(
        result.is_none(),
        "expected None when all completion lists are empty"
    );
}

/// Verifies that `close_completions_if_open` returns `Some(())` and clears
/// command completions when the command list is non-empty.
#[test]
fn close_completions_if_open_returns_some_and_clears_when_commands_non_empty() {
    let mut state = conversation_state();
    state.prompt.completions.commands = vec![FAKE_CMD];
    state.prompt.completions.command_selected = Some(0);

    let result = super::close_completions_if_open(&mut state);

    assert!(
        result.is_some(),
        "expected Some when command list is non-empty"
    );
    assert!(
        state.prompt.completions.commands.is_empty(),
        "commands must be cleared after close"
    );
    assert!(
        state.prompt.completions.command_selected.is_none(),
        "command_selected must be None after close"
    );
}

// ── apply_selected_completion - early-return paths ───────────────────────────

/// Verifies that `apply_selected_completion` leaves the buffer unchanged when
/// all completion lists are empty (no-op early-return path).
#[test]
fn apply_selected_completion_returns_early_when_no_completions() {
    let mut state = conversation_state();
    state.prompt.buffer = "hello".to_owned();
    state.prompt.cursor = 5;

    super::apply_selected_completion(&mut state);

    assert_eq!(
        state.prompt.buffer, "hello",
        "buffer must not change when no completions are active"
    );
    assert_eq!(state.prompt.cursor, 5, "cursor must remain unchanged");
}

// ── apply_selected_completion - command path ─────────────────────────────────

/// Verifies that `apply_selected_completion` writes `/name` into the buffer
/// when a command completion is selected.
#[test]
fn apply_selected_completion_command_path_sets_buffer_when_selected() {
    let mut state = conversation_state();
    state.prompt.completions.commands = vec![FAKE_CMD];
    state.prompt.completions.command_selected = Some(0);
    state.prompt.buffer = "/q".to_owned();

    super::apply_selected_completion(&mut state);

    assert_eq!(
        state.prompt.buffer, "/quit",
        "buffer must be set to /name of selected command"
    );
    assert_eq!(
        state.prompt.cursor,
        "/quit".len(),
        "cursor must be at end of inserted text"
    );
}

/// Verifies that `apply_selected_completion` does NOT modify the buffer when
/// the command list is non-empty but `command_selected` is `None`.
#[test]
fn apply_selected_completion_command_path_does_nothing_when_no_selection() {
    let mut state = conversation_state();
    state.prompt.completions.commands = vec![FAKE_CMD];
    state.prompt.completions.command_selected = None;
    state.prompt.buffer = "/q".to_owned();

    super::apply_selected_completion(&mut state);

    assert_eq!(
        state.prompt.buffer, "/q",
        "buffer must not change when command list is non-empty but nothing is selected"
    );
}

// ── apply_selected_completion - file path ────────────────────────────────────

/// Verifies that `apply_selected_completion` leaves the buffer unchanged when
/// the file list is non-empty but `file_selected` is `None` (no selection made).
#[test]
fn apply_selected_completion_file_path_does_nothing_when_no_file_selected() {
    let mut state = conversation_state();
    state.prompt.completions.files = vec![fake_file("src/main.rs")];
    state.prompt.completions.file_selected = None;
    state.prompt.buffer = "hello @m".to_owned();

    super::apply_selected_completion(&mut state);

    assert_eq!(
        state.prompt.buffer, "hello @m",
        "buffer must not change when files are present but none is selected"
    );
}

/// Verifies that `apply_selected_completion` expands the `@token` in the buffer
/// to the selected file path when `file_selected` is `Some`.
#[test]
fn apply_selected_completion_file_path_expands_at_token_when_file_selected() {
    let mut state = conversation_state();
    state.prompt.completions.files = vec![fake_file("src/lib.rs")];
    state.prompt.completions.file_selected = Some(0);
    state.prompt.buffer = "read @s".to_owned();

    super::apply_selected_completion(&mut state);

    assert!(
        state.prompt.buffer.contains("@src/lib.rs"),
        "buffer must contain the expanded file path after selection"
    );
    assert!(
        state.prompt.completions.files.is_empty(),
        "file completion list must be cleared after application"
    );
    assert!(
        state.prompt.completions.file_selected.is_none(),
        "file_selected must be reset to None after application"
    );
}

// ── apply_selected_completion - model path ───────────────────────────────────

/// Verifies that `apply_selected_completion` sets the buffer to exactly `/model`
/// when the selected model has an empty id (the Auto entry).
#[test]
fn apply_selected_completion_model_path_with_empty_id_sets_buffer_to_slash_model() {
    let mut state = conversation_state();
    state.prompt.completions.model_picker.items = vec![model_option("")];
    state.prompt.completions.model_picker.selected = Some(0);
    state.prompt.buffer = "/model".to_owned();

    super::apply_selected_completion(&mut state);

    assert_eq!(
        state.prompt.buffer, "/model",
        "empty model id must produce exactly /model in the buffer"
    );
    assert_eq!(state.prompt.cursor, "/model".len());
}

/// Verifies that `apply_selected_completion` writes `/model <id>` into the buffer
/// when a concrete model id is selected.
#[test]
fn apply_selected_completion_model_path_with_id_sets_buffer_to_model_id() {
    let mut state = conversation_state();
    state.prompt.completions.model_picker.items = vec![model_option("gpt-5")];
    state.prompt.completions.model_picker.selected = Some(0);
    state.prompt.buffer = "/model gp".to_owned();

    super::apply_selected_completion(&mut state);

    assert_eq!(
        state.prompt.buffer, "/model gpt-5",
        "selected model id must be written as /model <id>"
    );
    assert_eq!(state.prompt.cursor, "/model gpt-5".len());
}

/// Verifies that `apply_selected_completion` leaves the buffer unchanged when
/// the model picker list is non-empty but `model_picker.selected` is `None`.
#[test]
fn apply_selected_completion_model_path_does_nothing_when_no_model_selected() {
    let mut state = conversation_state();
    state.prompt.completions.model_picker.items = vec![model_option("gpt-5")];
    state.prompt.completions.model_picker.selected = None;
    state.prompt.buffer = "/model gp".to_owned();

    super::apply_selected_completion(&mut state);

    assert_eq!(
        state.prompt.buffer, "/model gp",
        "buffer must not change when model picker list is non-empty but nothing is selected"
    );
}

// ── refresh_completion_hints - routing / clearing ────────────────────────────

/// Verifies that `refresh_completion_hints` routes to the model picker path when
/// the buffer starts with `/model`, clearing command and file completion lists.
#[tokio::test]
async fn refresh_completion_hints_model_prefix_clears_commands_and_files() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    state.prompt.completions.commands = vec![FAKE_CMD];
    state.prompt.completions.command_selected = Some(0);
    state.prompt.completions.files = vec![fake_file("src/foo.rs")];
    state.prompt.completions.file_selected = Some(0);
    state.prompt.buffer = "/model".to_owned();

    super::refresh_completion_hints(&mut state, &rig.handles());

    assert!(
        state.prompt.completions.commands.is_empty(),
        "model-prefix path must clear the command completion list"
    );
    assert!(
        state.prompt.completions.command_selected.is_none(),
        "model-prefix path must clear command_selected"
    );
    assert!(
        state.prompt.completions.files.is_empty(),
        "model-prefix path must clear the file completion list"
    );
    assert!(
        state.prompt.completions.file_selected.is_none(),
        "model-prefix path must clear file_selected"
    );
    // model picker should be populated (at minimum the Auto option)
    assert!(
        !state.prompt.completions.model_picker.items.is_empty(),
        "model-prefix path must populate the model picker"
    );
}

/// Verifies that `refresh_completion_hints` routes to the clear-all path when
/// the buffer is plain text (no `/` prefix and no `@`), wiping all completion lists.
#[tokio::test]
async fn refresh_completion_hints_plain_buffer_clears_all_completions() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    state.prompt.completions.commands = vec![FAKE_CMD];
    state.prompt.completions.model_picker.items = vec![model_option("gpt-5")];
    state.prompt.completions.files = vec![fake_file("src/foo.rs")];
    state.prompt.buffer = "hello world".to_owned();

    super::refresh_completion_hints(&mut state, &rig.handles());

    assert!(
        state.prompt.completions.commands.is_empty(),
        "plain-text buffer must clear command completions"
    );
    assert!(
        state.prompt.completions.files.is_empty(),
        "plain-text buffer must clear file completions"
    );
    assert!(
        state.prompt.completions.model_picker.items.is_empty(),
        "plain-text buffer must clear model picker completions"
    );
}

/// Verifies that `/run-pipeline @…` routes to the file-completion branch even
/// though the buffer starts with `/`, so attachment autocomplete works.
#[tokio::test]
async fn refresh_completion_hints_run_pipeline_with_at_shows_file_completions() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    // Pre-populate command completions to confirm they are cleared.
    state.prompt.completions.commands = vec![FAKE_CMD];
    state.prompt.buffer = "/run-pipeline @src".to_owned();

    super::refresh_completion_hints(&mut state, &rig.handles());

    assert!(
        state.prompt.completions.commands.is_empty(),
        "/run-pipeline @… must clear command completions in favour of file completions"
    );
}

/// Verifies that a bare `/run-pipeline` (no `@`) still routes to the
/// command-completion branch so the command itself appears in the picker.
#[tokio::test]
async fn refresh_completion_hints_run_pipeline_without_at_shows_command_completions() {
    let rig = TestRig::new().await;
    let mut state = conversation_state();
    state.prompt.completions.files = vec![fake_file("plans/foo.md")];
    state.prompt.buffer = "/run-pipeline".to_owned();

    super::refresh_completion_hints(&mut state, &rig.handles());

    assert!(
        state.prompt.completions.files.is_empty(),
        "bare /run-pipeline must clear file completions and stay in command branch"
    );
}
