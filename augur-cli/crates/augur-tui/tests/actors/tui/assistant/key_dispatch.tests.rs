use crate::domain::string_newtypes::{
    EndpointName, FilePath, ModelLabel, PromptText, StringNewtype,
};
use crate::domain::traits::ChatProvider;
use crate::domain::tui_state::{AppScreen, AppState};
use crate::domain::types::AgentOutput;
use crate::persistence::types::MessageRecord;
use std::sync::{Arc, Mutex};

use crate::tests::helpers::fake_ask;

fn model_option(id: &str, display_name: &str) -> crate::domain::types::ModelOption {
    crate::domain::types::ModelOption::builder()
        .id(crate::domain::string_newtypes::ModelId::new(id))
        .display_name(ModelLabel::new(display_name))
        .build()
}

fn command_def(
    name: &'static str,
    usage: &'static str,
    description: &'static str,
) -> crate::domain::types::CommandDef {
    crate::domain::types::CommandDef::builder()
        .name(name)
        .usage(usage)
        .description(description)
        .build()
}

/// Verifies that close_completions_if_open returns false and leaves state
/// unchanged when there are no completions open.
#[test]
fn close_completions_noop_when_empty() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let closed = super::close_completions_if_open(&mut state);
    assert!(
        closed.is_none(),
        "must return false when completions are already empty"
    );
    assert!(state.prompt.completions.is_empty().0);
}

/// Verifies that refresh_file_hints writes scan results from the file scanner
/// into state.prompt.completions.files when the scanner returns matches.
///
/// Spawns the real FileScannerActor, triggers a scan for a known directory,
/// waits for the actor to process it, then asserts the hint list is populated.
#[tokio::test]
async fn refresh_file_hints_populates_file_completions() {
    let (join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    // scan for "src" prefix - the project's src/ directory exists at cwd
    scanner.scan("src");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "@src".to_owned();

    super::refresh_file_hints(&mut state, &scanner);

    assert!(
        !state.prompt.completions.files.is_empty(),
        "file completions must be populated after a scan of 'src'"
    );
    scanner.shutdown();
    let _ = join.await;
}

/// Verifies that close_completions_if_open also clears the model_picker.
///
/// When the model picker has items, close_completions_if_open must clear
/// them and return true so pressing Esc dismisses the model picker.
#[test]
fn close_completions_clears_model_picker() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.completions.model_picker.items = vec![model_option("gpt-4o", "GPT-4o")];
    state.prompt.completions.model_picker.selected = Some(0);
    let closed = super::close_completions_if_open(&mut state);
    assert!(
        closed.is_some(),
        "must return true when model picker was open"
    );
    assert!(state.prompt.completions.model_picker.items.is_empty());
    assert!(state.prompt.completions.model_picker.selected.is_none());
}

/// Verifies that history navigation suppresses completion refresh while already
/// in history mode, even when the recalled entry starts with `/`.
#[test]
fn should_skip_completion_refresh_for_repeated_history_up() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.history.pos = Some(0);
    state.prompt.buffer = "/model gpt-5".to_owned();
    state.prompt.completions.commands = vec![command_def("/model", "/model", "model picker")];

    assert!(
        super::should_skip_completion_refresh(
            &state,
            &crate::domain::tui_input::KeyAction::CompletionUp,
        ),
        "repeated Up during history navigation must skip completion refresh"
    );
    assert!(
        super::should_skip_completion_refresh(
            &state,
            &crate::domain::tui_input::KeyAction::CompletionDown,
        ),
        "Down during history navigation must also skip completion refresh"
    );
    assert!(
        !super::should_skip_completion_refresh(&state, &crate::domain::tui_input::KeyAction::Tab,),
        "non-history actions must not skip completion refresh"
    );
}

/// Verifies that refresh_model_hints populates the model_picker from available models.
///
/// When the buffer is "/model " (with a space), all available models plus the
/// Auto option must be shown in the model_picker hint list.
#[test]
fn refresh_model_hints_populates_from_available_models() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![
        model_option("gpt-4o", "GPT-4o"),
        model_option("claude-3-5-sonnet", "Claude 3.5 Sonnet"),
    ];
    state.prompt.buffer = "/model ".to_owned();
    super::refresh_model_hints(&mut state);
    // 2 models + 1 Auto option = 3
    assert_eq!(state.prompt.completions.model_picker.items.len(), 3);
}

/// Verifies that refresh_model_hints filters by id prefix.
///
/// When the buffer is "/model gpt", only models whose id starts with "gpt"
/// should appear in the model_picker, plus the Auto option always at index 0.
#[test]
fn refresh_model_hints_filters_by_prefix() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![
        model_option("gpt-4o", "GPT-4o"),
        model_option("claude-3-5-sonnet", "Claude 3.5 Sonnet"),
    ];
    state.prompt.buffer = "/model gpt".to_owned();
    super::refresh_model_hints(&mut state);
    // Auto is not shown when filtering by a non-empty prefix that doesn't match ""
    // Only gpt-4o matches the "gpt" prefix; Auto has id "" which doesn't start with "gpt"
    assert_eq!(state.prompt.completions.model_picker.items.len(), 1);
    assert_eq!(
        state.prompt.completions.model_picker.items[0].id.as_str(),
        "gpt-4o"
    );
}

/// Verifies that refresh_model_hints pre-selects item 0 when the list changes and no active model.
///
/// If the model picker list changes (e.g., user opens picker for the first time),
/// and no active_id is set, selection must be pre-set to Some(0) - the Auto option
/// at index 0 - so the user can immediately press Enter to confirm auto-selection.
#[test]
fn refresh_model_hints_resets_selection_on_list_change() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![model_option("gpt-4o", "GPT-4o")];
    state.prompt.buffer = "/model ".to_owned();
    // Prime picker with a different model list to force a change
    state.prompt.completions.model_picker.items = vec![model_option("old-model", "Old")];
    state.prompt.completions.model_picker.selected = Some(0);
    super::refresh_model_hints(&mut state);
    // When list changes with no active_id, pre-selects index 0 (Auto)
    assert_eq!(state.prompt.completions.model_picker.selected, Some(0));
}

/// Verifies that refresh_file_hints resets file_selected to None when the
/// file completion list changes between calls.
#[tokio::test]
async fn refresh_file_hints_resets_selection_on_list_change() {
    let (join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "@src".to_owned();
    // Set a stale selection on a previously different list
    state.prompt.completions.file_selected = Some(99);

    scanner.scan("src");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    super::refresh_file_hints(&mut state, &scanner);

    assert_eq!(
        state.prompt.completions.file_selected, None,
        "file_selected must reset to None when the completion list changes"
    );
    scanner.shutdown();
    let _ = join.await;
}

/// Verifies that refresh_model_hints always prepends an Auto option at index 0.
///
/// The Auto sentinel (id = "") must appear first in every model picker list so
/// the user can always press Enter immediately to revert to CLI auto-selection.
#[test]
fn refresh_model_hints_prepends_auto_option() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![model_option("gpt-4o", "GPT-4o")];
    state.prompt.buffer = "/model ".to_owned();
    super::refresh_model_hints(&mut state);
    let items = &state.prompt.completions.model_picker.items;
    assert!(!items.is_empty(), "picker must not be empty");
    assert_eq!(
        items[0].id.as_str(),
        "",
        "first item must be the Auto sentinel (empty id)"
    );
    assert_eq!(
        items[0].display_name, "Auto",
        "first item must have display_name 'Auto'"
    );
}

/// Verifies that refresh_model_hints pre-selects the active model when active_id is set.
///
/// When the user opens the model picker and an active model is already set, the
/// picker must highlight that model so the user can see what is currently active.
#[test]
fn refresh_model_hints_pre_selects_active_model() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![
        model_option("gpt-4o", "GPT-4o"),
        model_option("claude-3-5-sonnet", "Claude 3.5 Sonnet"),
    ];
    state.prompt.models.active_id = Some(crate::domain::string_newtypes::ModelId::new(
        "claude-3-5-sonnet",
    ));
    state.prompt.buffer = "/model ".to_owned();
    super::refresh_model_hints(&mut state);
    // Auto is at 0, gpt-4o at 1, claude-3-5-sonnet at 2
    let selected = state.prompt.completions.model_picker.selected;
    let items = &state.prompt.completions.model_picker.items;
    let active_idx = items
        .iter()
        .position(|m| m.id.as_str() == "claude-3-5-sonnet");
    assert_eq!(
        selected, active_idx,
        "picker must pre-select the active model"
    );
}

/// Verifies that bare "/model" buffer (no space) triggers the model picker.
///
/// When the buffer equals "/model" exactly (no trailing space), refresh_model_hints
/// must still populate the picker with all models so the picker appears immediately
/// when the user finishes typing "/model".
#[test]
fn refresh_model_hints_bare_model_shows_all_models() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![model_option("gpt-4o", "GPT-4o")];
    state.prompt.buffer = "/model".to_owned();
    super::refresh_model_hints(&mut state);
    // Auto + gpt-4o = 2 items
    assert_eq!(
        state.prompt.completions.model_picker.items.len(),
        2,
        "bare /model must show all available models plus Auto"
    );
}

/// Verifies that refresh_model_hints filters by substring of model id or display name.
///
/// Typing "sonnet" must match "claude-3-5-sonnet" even though it is not a prefix,
/// because the filter must use contains() rather than starts_with().
#[test]
fn refresh_model_hints_filters_by_substring() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![
        model_option("gpt-4o", "GPT-4o"),
        model_option("claude-3-5-sonnet", "Claude 3.5 Sonnet"),
    ];
    state.prompt.buffer = "/model sonnet".to_owned();
    super::refresh_model_hints(&mut state);
    assert_eq!(
        state.prompt.completions.model_picker.items.len(),
        1,
        "only claude-3-5-sonnet must match substring 'sonnet'"
    );
    assert_eq!(
        state.prompt.completions.model_picker.items[0].id.as_str(),
        "claude-3-5-sonnet"
    );
}

/// Verifies that refresh_model_hints filters case-insensitively.
///
/// Typing "CLAUDE" (uppercase) must still match "claude-3-5-sonnet" and its
/// display name "Claude 3.5 Sonnet" regardless of case.
#[test]
fn refresh_model_hints_filters_case_insensitively() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![
        model_option("gpt-4o", "GPT-4o"),
        model_option("claude-3-5-sonnet", "Claude 3.5 Sonnet"),
    ];
    state.prompt.buffer = "/model CLAUDE".to_owned();
    super::refresh_model_hints(&mut state);
    assert_eq!(
        state.prompt.completions.model_picker.items.len(),
        1,
        "only claude-3-5-sonnet must match case-insensitive 'CLAUDE'"
    );
    assert_eq!(
        state.prompt.completions.model_picker.items[0].id.as_str(),
        "claude-3-5-sonnet"
    );
}

/// Verifies that refresh_model_hints matches Gemini ids and display names.
///
/// Model ids from Copilot are passed through from the SDK, so a Gemini model
/// such as `"gemini-3.1-pro"` with display name `"Gemini 3.1 Pro"` must be
/// discoverable from either the id or the user-facing name.
#[test]
fn refresh_model_hints_matches_gemini_model() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.models.available = vec![
        model_option("gpt-4o", "GPT-4o"),
        model_option("gemini-3.1-pro", "Gemini 3.1 Pro"),
    ];

    state.prompt.buffer = "/model gemini".to_owned();
    super::refresh_model_hints(&mut state);
    assert_eq!(state.prompt.completions.model_picker.items.len(), 1);
    assert_eq!(
        state.prompt.completions.model_picker.items[0].id.as_str(),
        "gemini-3.1-pro"
    );

    state.prompt.buffer = "/model 3.1 pro".to_owned();
    super::refresh_model_hints(&mut state);
    assert_eq!(state.prompt.completions.model_picker.items.len(), 1);
    assert_eq!(
        state.prompt.completions.model_picker.items[0].display_name,
        "Gemini 3.1 Pro"
    );
}

/// Verifies that apply_selected_completion sets buffer to "/model" when Auto is selected.
///
/// Selecting the Auto sentinel (id = "") from the model picker must produce a bare
/// "/model" buffer so that handle_submit receives the bare command and routes it to
/// SelectAutoModel, triggering CLI auto-selection.
#[test]
fn apply_selected_completion_auto_sets_bare_model_buffer() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.completions.model_picker.items =
        vec![model_option("", "Auto"), model_option("gpt-4o", "GPT-4o")];
    state.prompt.completions.model_picker.selected = Some(0);
    super::apply_selected_completion(&mut state);
    assert_eq!(
        state.prompt.buffer, "/model",
        "selecting Auto must set buffer to bare /model"
    );
    assert_eq!(state.prompt.cursor, "/model".len());
}

// --- ChatProvider routing tests -----------------------------------------------

/// Captures calls made to the `ChatProvider` submit methods.
///
/// Each recorded call carries the prompt and, for attachment calls, the
/// attachment list. Used by `handle_submit` routing tests to distinguish
/// plain-submit from submit-with-attachments dispatch.
#[derive(Debug, PartialEq)]
enum ProviderCall {
    Submit {
        prompt: PromptText,
    },
    SubmitWithAttachments {
        prompt: PromptText,
        attachments: Vec<FilePath>,
    },
}

/// `(model_id_str, Option<effort_str>)` pairs recorded by `set_model_with_options`.
type ModelOptionsCall = (String, Option<String>);

/// Test double for `ChatProvider` that records routing decisions.
///
/// Stores calls in an `Arc<Mutex<Vec<ProviderCall>>>` so ownership can be shared
/// between the provider reference held by `TuiHandles` and the assertion site.
/// Also records `set_model_with_options` calls as `(model_id_str, Option<effort_str>)`.
struct RecordingChatProvider {
    calls: Arc<Mutex<Vec<ProviderCall>>>,
    set_model_options_calls: Arc<Mutex<Vec<ModelOptionsCall>>>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
}

impl RecordingChatProvider {
    /// Constructs a fresh provider with an empty call log.
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            set_model_options_calls: Arc::new(Mutex::new(Vec::new())),
            output_tx,
        }
    }

    /// Drains and returns all recorded calls since construction or last drain.
    fn take_calls(&self) -> Vec<ProviderCall> {
        self.calls.lock().unwrap().drain(..).collect()
    }

    /// Drains and returns all `set_model_with_options` calls since construction or last drain.
    fn take_set_model_options_calls(&self) -> Vec<ModelOptionsCall> {
        self.set_model_options_calls
            .lock()
            .unwrap()
            .drain(..)
            .collect()
    }
}

impl ChatProvider for RecordingChatProvider {
    fn submit(&self, prompt: PromptText, _endpoint: Option<EndpointName>) {
        self.calls
            .lock()
            .unwrap()
            .push(ProviderCall::Submit { prompt });
    }

    fn submit_with_attachments(
        &self,
        prompt: PromptText,
        _endpoint: Option<EndpointName>,
        attachments: Vec<FilePath>,
    ) {
        self.calls
            .lock()
            .unwrap()
            .push(ProviderCall::SubmitWithAttachments {
                prompt,
                attachments,
            });
    }

    fn set_model_with_options(
        &self,
        model_id: crate::domain::string_newtypes::ModelId,
        reasoning_effort: Option<crate::domain::thinking_mode::ReasoningEffort>,
    ) {
        self.set_model_options_calls.lock().unwrap().push((
            model_id.to_string(),
            reasoning_effort.map(|e| e.as_ref().to_owned()),
        ));
    }

    fn interrupt(&self) {}
    fn shutdown(&self) {}
    fn restore(&self, _records: Vec<MessageRecord>) {}

    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }
}

/// Verifies that handle_submit routes to submit_with_attachments when the
/// buffer contains an @path token, passing the resolved FilePath list and
/// the cleaned prompt text with the @token removed.
#[tokio::test]
async fn handle_submit_with_at_token_calls_submit_with_attachments() {
    let provider = RecordingChatProvider::new();
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

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "@src/main.rs explain this".to_owned();

    let should_quit = super::handle_submit(&mut state, &handles).await;
    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "submit with attachments must not quit"
    );

    let calls = provider.take_calls();
    assert_eq!(calls.len(), 1, "exactly one provider call must be recorded");
    match &calls[0] {
        ProviderCall::SubmitWithAttachments {
            prompt,
            attachments,
        } => {
            assert_eq!(
                prompt.as_str(),
                "explain this",
                "clean prompt must strip the @token"
            );
            assert_eq!(
                attachments,
                &[FilePath::new("src/main.rs")],
                "attachment list must contain the resolved path"
            );
        }
        other => panic!("expected SubmitWithAttachments, got {:?}", other),
    }
}

/// Verifies that handle_submit routes to plain submit when the buffer
/// contains no @path tokens, leaving the full prompt text unchanged.
#[tokio::test]
async fn handle_submit_without_at_tokens_calls_plain_submit() {
    let provider = RecordingChatProvider::new();
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

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "explain this without any attachment".to_owned();

    let should_quit = super::handle_submit(&mut state, &handles).await;
    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "plain submit must not quit"
    );

    let calls = provider.take_calls();
    assert_eq!(calls.len(), 1, "exactly one provider call must be recorded");
    match &calls[0] {
        ProviderCall::Submit { .. } => {}
        other => panic!("expected plain Submit, got {:?}", other),
    }
}

/// Verifies that handle_submit for "/new-session" clears accumulated output
/// lines and adds a system message confirming the new session started.
///
/// After /new-session the output pane must be clean (no previous conversation
/// visible) and the user must see a confirmation that a new session was started.
#[tokio::test]
async fn handle_submit_new_session_clears_output_and_starts_fresh() {
    use crate::domain::string_newtypes::OutputText;
    use crate::domain::tui_state::{AskPanelState, InputFocus};
    let provider = RecordingChatProvider::new();
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

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state
        .output
        .lines
        .push(crate::domain::tui_state::OutputLine::plain(
            OutputText::new("old conversation"),
        ));
    state.status.token_totals.tokens_in = crate::domain::TokenCount::of(123);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Ask;
    state.prompt.buffer = "/new-session".to_owned();

    let should_quit = super::handle_submit(&mut state, &handles).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "/new-session must not quit the TUI"
    );
    let output_text: String = state
        .output
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        !output_text.contains("old conversation"),
        "output must be cleared after /new-session, got: {output_text:?}"
    );
    assert!(
        output_text.contains("new session"),
        "system message about new session must appear: got {output_text:?}"
    );
    assert_eq!(
        state.status.token_totals.tokens_in,
        crate::domain::TokenCount::of(0),
        "/new-session must clear displayed token totals"
    );
    assert!(
        state.interaction.panel.ask_panel.is_none(),
        "/new-session must clear any open ask panel state"
    );
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Main,
        "/new-session must restore focus to the main input",
    );
}

/// Verifies that toggle_ask_focus flips input_focus from Main to Ask when ask panel is open.
///
/// When ask_panel is Some, calling toggle_ask_focus must change input_focus to Ask.
#[test]
fn toggle_ask_focus_main_to_ask_when_panel_open() {
    use crate::domain::tui_state::{AskPanelState, InputFocus};
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Main;
    super::toggle_ask_focus(&mut state);
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Ask);
}

/// Verifies that toggle_ask_focus flips input_focus from Ask to Main when ask panel is open.
///
/// When focus is Ask, toggle_ask_focus must return focus to Main.
#[test]
fn toggle_ask_focus_ask_to_main_when_panel_open() {
    use crate::domain::tui_state::{AskPanelState, InputFocus};
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Ask;
    super::toggle_ask_focus(&mut state);
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Main);
}

/// Verifies that toggle_ask_focus is a no-op when ask panel is closed.
///
/// When ask_panel is None, input_focus must remain Main regardless of the call.
#[test]
fn toggle_ask_focus_noop_when_panel_closed() {
    use crate::domain::tui_state::InputFocus;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    assert!(state.interaction.panel.ask_panel.is_none());
    state.interaction.panel.input_focus = InputFocus::Main;
    super::toggle_ask_focus(&mut state);
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Main);
}

/// Verifies that dispatch_plan_esc transitions from Plan to Chat mode when
/// no completions are open and the agent is not thinking.
///
/// Pressing Esc in Plan mode with idle state must set mode to Chat.
#[test]
fn dispatch_plan_esc_transitions_to_chat() {
    use crate::domain::plan_tree::PlanTree;
    use crate::domain::tui_state::{ConversationMode, PlanModeState};
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree: PlanTree::new("test", "test", "test"),
        running: false,
        tree_scroll: crate::domain::newtypes::ScrollOffset::of(0),
    });
    super::dispatch_plan_esc(&mut state);
    assert!(
        matches!(state.interaction.mode, ConversationMode::Chat),
        "must transition to Chat mode on Esc"
    );
}

/// Verifies that dispatch_plan_esc is a no-op when completions are open.
///
/// When any completion list is populated, Esc must close completions first,
/// not exit plan mode - the caller handles the two-press pattern.
#[test]
fn dispatch_plan_esc_noop_when_completions_open() {
    use crate::domain::plan_tree::PlanTree;
    use crate::domain::tui_state::{ConversationMode, PlanModeState};
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree: PlanTree::new("test", "test", "test"),
        running: false,
        tree_scroll: crate::domain::newtypes::ScrollOffset::of(0),
    });
    state.prompt.completions.commands = vec![command_def("ask", "/ask", "open ask panel")];
    super::dispatch_plan_esc(&mut state);
    assert!(
        matches!(state.interaction.mode, ConversationMode::Plan(_)),
        "must remain in Plan mode when completions are open"
    );
}

/// Verifies that dispatch_plan_esc is a no-op when the agent is thinking.
///
/// When agent is actively thinking, Esc in plan mode must not exit to Chat -
/// it should be handled by the normal CancelThinking flow instead.
#[test]
fn dispatch_plan_esc_noop_when_thinking() {
    use crate::domain::plan_tree::PlanTree;
    use crate::domain::tui_state::{ConversationMode, PlanModeState};
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.mode = ConversationMode::Plan(PlanModeState {
        tree: PlanTree::new("test", "test", "test"),
        running: false,
        tree_scroll: crate::domain::newtypes::ScrollOffset::of(0),
    });
    state.agent.thinking.is_active = true;
    super::dispatch_plan_esc(&mut state);
    assert!(
        matches!(state.interaction.mode, ConversationMode::Plan(_)),
        "must remain in Plan mode when agent is thinking"
    );
}

/// Verifies that Esc with ask focus active closes the secondary view and switches to Main.
///
/// When `secondary_view` is Some and `input_focus == Ask`, pressing Esc must
/// close `secondary_view` (set to None) and reset `input_focus` to Main.
/// The ask panel state is preserved (not cleared) so the conversation is available
/// on next open.
#[tokio::test]
async fn esc_with_ask_focus_switches_to_main_focus() {
    use crate::domain::tui_state::{AskPanelState, InputFocus, SecondaryView};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Ask;
    let key = KeyEvent {
        code: KeyCode::Esc,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Main,
        "Esc must reset focus to Main"
    );
    assert!(
        state.interaction.panel.secondary_view.is_none(),
        "Esc must close secondary_view"
    );
    assert!(
        state.interaction.panel.ask_panel.is_some(),
        "panel state must be preserved when Esc closes secondary view"
    );
}

/// Verifies that Esc with main focus and secondary view open closes the secondary view.
///
/// When `secondary_view` is `Some(Ask)` and `input_focus == Main`, pressing Esc must
/// set `secondary_view` to None and keep `input_focus` as Main.
/// The ask panel state is preserved so the conversation persists.
#[tokio::test]
async fn esc_with_main_focus_closes_ask_panel() {
    use crate::domain::tui_state::{AskPanelState, InputFocus, SecondaryView};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Main;
    state.agent.thinking.is_active = false;
    let key = KeyEvent {
        code: KeyCode::Esc,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert!(
        state.interaction.panel.secondary_view.is_none(),
        "Esc must close secondary_view"
    );
    assert!(
        state.interaction.panel.ask_panel.is_some(),
        "ask panel state must be preserved on Esc"
    );
}

/// Verifies that ShiftTab opens the ask panel and sets focus to Ask when panel is closed.
///
/// Pressing Shift+Tab when ask_panel is None must create an AskPanelState and
/// switch input_focus to Ask.
#[tokio::test]
async fn shift_tab_opens_ask_panel_and_sets_ask_focus() {
    use crate::domain::tui_state::InputFocus;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    assert!(state.interaction.panel.ask_panel.is_none());
    // BackTab is crossterm's encoding for Shift+Tab
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert!(
        state.interaction.panel.ask_panel.is_some(),
        "Shift+Tab must open ask panel"
    );
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Ask,
        "Shift+Tab must set focus to Ask"
    );
}

/// Verifies that ShiftTab closes the secondary view when ask view is already open.
///
/// With `secondary_view = Some(Ask)`, a second Shift+Tab press must close the
/// secondary view (`secondary_view = None`) and reset focus to Main.
#[tokio::test]
async fn shift_tab_noop_when_panel_already_open() {
    use crate::domain::tui_state::{AskPanelState, InputFocus, SecondaryView};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Ask;
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert!(
        state.interaction.panel.secondary_view.is_none(),
        "ShiftTab when ask is open must close secondary view"
    );
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Main,
        "ShiftTab close must reset focus to Main"
    );
    assert!(
        state.interaction.panel.ask_panel.is_some(),
        "ask panel state must be preserved on close"
    );
}

/// Verifies that Tab toggles input_focus from Main to Ask when ask panel is open.
///
/// With ask_panel open and focus on Main, Tab must switch focus to Ask.
#[tokio::test]
async fn tab_toggles_focus_when_panel_open() {
    use crate::domain::tui_state::{AskPanelState, InputFocus};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Main;
    let key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Ask,
        "Tab must toggle focus to Ask when panel open"
    );
}

/// Verifies that Tab autocompletes the selected `@` file inline instead of submitting.
///
/// When the file picker is visible and a file is selected, pressing Tab must
/// replace the in-progress `@token` inside the prompt buffer, leave the rest of
/// the typed text intact, keep focus on the main input, and record no provider
/// submit call.
#[tokio::test]
async fn tab_with_visible_file_picker_completes_inline_without_submitting() {
    use crate::domain::tui_state::{AskPanelState, InputFocus};
    use crate::domain::types::FileCompletion;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.input_focus = InputFocus::Main;
    state.prompt.buffer = "inspect @sr now".to_owned();
    state.prompt.cursor = "inspect @sr".len();
    state.prompt.completions.files = vec![FileCompletion {
        path: FilePath::new("src/main.rs"),
        display_name: "main.rs".to_owned().into(),
    }];
    state.prompt.completions.file_selected = Some(0);

    let key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let should_quit = super::dispatch_chat_key(&mut state, key, &handles).await;

    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "Tab autocomplete must not quit"
    );
    assert_eq!(state.prompt.buffer, "inspect @src/main.rs now");
    assert_eq!(state.prompt.cursor, "inspect @src/main.rs".len());
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Main,
        "Tab must autocomplete instead of toggling focus when file picker is visible"
    );
    assert!(
        provider.take_calls().is_empty(),
        "Tab autocomplete while the file picker is visible must not submit the message"
    );
}

// ── Phase 3: secondary view toggle tests ─────────────────────────────────────

/// Verifies that ShiftTab with secondary_view = None opens the ask secondary view.
///
/// When no secondary view is active, ShiftTab must set `secondary_view = Some(Ask)`,
/// create an `ask_panel`, and set `input_focus = Ask`.
#[tokio::test]
async fn secondary_view_toggle_shifttab_opens_ask_when_closed() {
    use crate::domain::tui_state::{InputFocus, SecondaryView};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    assert!(state.interaction.panel.secondary_view.is_none());
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::Ask),
        "ShiftTab when closed must open ask secondary view",
    );
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Ask);
    assert!(
        state.interaction.panel.ask_panel.is_some(),
        "ask panel must be initialized"
    );
}

/// Verifies that ShiftTab with secondary_view = Some(Ask) closes the secondary view.
///
/// When the ask view is open, ShiftTab must set `secondary_view = None` and
/// reset `input_focus = Main`. The ask panel state is preserved.
#[tokio::test]
async fn secondary_view_toggle_shifttab_closes_when_ask_open() {
    use crate::domain::tui_state::{AskPanelState, InputFocus, SecondaryView};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Ask;
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert!(
        state.interaction.panel.secondary_view.is_none(),
        "ShiftTab when ask is open must close secondary view",
    );
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Main);
}

/// Verifies that Ctrl+T with secondary_view = None opens the agent feed view.
///
/// When no secondary view is active, Ctrl+T must set `secondary_view = Some(AgentFeed)`.
#[tokio::test]
async fn secondary_view_toggle_ctrl_t_opens_agent_feed_when_closed() {
    use crate::domain::tui_state::SecondaryView;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    assert!(state.interaction.panel.secondary_view.is_none());
    let key = KeyEvent {
        code: KeyCode::Char('t'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::AgentFeed),
        "Ctrl+T when closed must open agent feed secondary view",
    );
}

/// Verifies that ShiftTab with secondary_view = Some(AgentFeed) switches to Ask.
///
/// When the agent feed is open, ShiftTab must set `secondary_view = Some(Ask)`,
/// initialize the ask panel if needed, and set `input_focus = Ask`.
#[tokio::test]
async fn secondary_view_toggle_shiftab_switches_to_ask_when_agent_feed_open() {
    use crate::domain::tui_state::{InputFocus, SecondaryView};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.secondary_view = Some(SecondaryView::AgentFeed);
    let key = KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::Ask),
        "ShiftTab when AgentFeed is open must switch secondary_view to Ask",
    );
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Ask,
        "ShiftTab when AgentFeed is open must set input_focus to Ask",
    );
    assert!(
        state.interaction.panel.ask_panel.is_some(),
        "ShiftTab switching from AgentFeed must initialize ask_panel",
    );
}

/// Verifies that Ctrl+W closes the currently open secondary panel.
///
/// When `secondary_view` is `Some(AgentFeed)`, dispatching a Ctrl+W key event
/// must set `secondary_view` to `None`.
#[tokio::test]
async fn close_secondary_panel_key_closes_panel() {
    use crate::domain::tui_state::SecondaryView;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.secondary_view = Some(SecondaryView::AgentFeed);
    let key = KeyEvent {
        code: KeyCode::Char('w'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert!(
        state.interaction.panel.secondary_view.is_none(),
        "Ctrl+W must close the secondary panel (set secondary_view to None)",
    );
}

/// Verifies that Ctrl+W while the Ask panel is open resets `input_focus` to Main.
///
/// Regression: `CloseSecondaryPanel` was clearing `secondary_view` but not resetting
/// `input_focus`, leaving it as `InputFocus::Ask`. Every subsequent Enter keypress
/// would then silently route to the now-hidden Ask panel instead of the main chat.
///
/// This test asserts both invariants that every secondary-panel close path must uphold:
/// `secondary_view` is `None` AND `input_focus` is `Main`.
#[tokio::test]
async fn ctrl_w_closes_ask_panel_and_resets_input_focus() {
    use crate::domain::tui_state::{AskPanelState, InputFocus, SecondaryView};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Ask;
    let key = KeyEvent {
        code: KeyCode::Char('w'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;
    assert!(
        state.interaction.panel.secondary_view.is_none(),
        "Ctrl+W must close secondary_view",
    );
    assert_eq!(
        state.interaction.panel.input_focus,
        InputFocus::Main,
        "Ctrl+W must reset input_focus to Main",
    );
}

/// Verifies that agent-feed selection moves right when multiple feeds are present.
#[test]
fn select_next_agent_feed_advances_selection() {
    use crate::domain::string_newtypes::ToolCallId;
    use crate::domain::tui_state::{AgentFeedState, AgentFeedTranscript, SecondaryView};
    use crate::domain::types::FeedId;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.secondary_view = Some(SecondaryView::AgentFeed);
    state.interaction.panel.agent_feed = AgentFeedState {
        feeds: vec![
            AgentFeedTranscript {
                feed_id: FeedId::Agent(ToolCallId::from("agent-1")),
                ..Default::default()
            },
            AgentFeedTranscript {
                feed_id: FeedId::Agent(ToolCallId::from("agent-2")),
                ..Default::default()
            },
        ],
        selected_feed: Some(0),
        ..Default::default()
    };
    assert!(bool::from(state.select_next_agent_feed()));
    assert_eq!(state.interaction.panel.agent_feed.selected_feed, Some(1));
}

/// Verifies that agent-feed selection moves left when multiple feeds are present.
#[test]
fn select_prev_agent_feed_moves_back() {
    use crate::domain::string_newtypes::ToolCallId;
    use crate::domain::tui_state::{AgentFeedState, AgentFeedTranscript, SecondaryView};
    use crate::domain::types::FeedId;

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.secondary_view = Some(SecondaryView::AgentFeed);
    state.interaction.panel.agent_feed = AgentFeedState {
        feeds: vec![
            AgentFeedTranscript {
                feed_id: FeedId::Agent(ToolCallId::from("agent-1")),
                ..Default::default()
            },
            AgentFeedTranscript {
                feed_id: FeedId::Agent(ToolCallId::from("agent-2")),
                ..Default::default()
            },
        ],
        selected_feed: Some(1),
        ..Default::default()
    };
    assert!(bool::from(state.select_prev_agent_feed()));
    assert_eq!(state.interaction.panel.agent_feed.selected_feed, Some(0));
}

// --- RunBackgroundAgent arm test ----------------------------------------------

/// Records calls to `run_background_agent` so the routing test can assert the
/// correct arguments were forwarded to the provider.
struct RecordingBgAgentProvider {
    calls: Arc<Mutex<Vec<(String, String)>>>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
}

impl RecordingBgAgentProvider {
    fn new() -> Self {
        let (output_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            output_tx,
        }
    }

    fn take_calls(&self) -> Vec<(String, String)> {
        self.calls.lock().unwrap().drain(..).collect()
    }
}

impl ChatProvider for RecordingBgAgentProvider {
    fn submit(&self, _prompt: PromptText, _endpoint: Option<EndpointName>) {}

    fn submit_with_attachments(
        &self,
        _prompt: PromptText,
        _endpoint: Option<EndpointName>,
        _attachments: Vec<FilePath>,
    ) {
    }

    fn interrupt(&self) {}
    fn shutdown(&self) {}
    fn restore(&self, _records: Vec<MessageRecord>) {}

    fn subscribe_output(&self) -> tokio::sync::broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    fn run_background_agent(&self, agent: crate::domain::AgentName, prompt: PromptText) {
        self.calls
            .lock()
            .unwrap()
            .push((agent.to_string(), prompt.to_string()));
    }
}

/// Verifies that handle_submit calls provider.run_background_agent(agent, prompt)
/// when the command outcome is RunBackgroundAgent.
///
/// The `/agent copilot go` buffer resolves to
/// `CommandOutcome::RunBackgroundAgent { agent: "copilot", prompt: "go" }`.
/// The provider must record exactly one call with those arguments.
/// This test fails (panics) while the `todo!("Phase 4")` stub is in place.
#[tokio::test]
async fn handle_submit_run_background_agent_calls_provider() {
    let provider = RecordingBgAgentProvider::new();
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

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/agent copilot go".to_owned();

    let should_quit = super::handle_submit(&mut state, &handles).await;
    assert!(
        matches!(should_quit, std::ops::ControlFlow::Continue(())),
        "background agent submit must not quit"
    );

    let calls = provider.take_calls();
    assert_eq!(
        calls.len(),
        1,
        "exactly one run_background_agent call must be recorded"
    );
    assert_eq!(
        calls[0],
        ("copilot".to_owned(), "go".to_owned()),
        "run_background_agent must be called with (agent, prompt) from the command"
    );
}

/// Verifies that the first Ask-panel open restores only user/assistant/system
/// lines into the ask session, and a later reopen does not restore again.
#[tokio::test]
async fn first_ask_panel_open_restores_filtered_main_snapshot_once() {
    use crate::domain::newtypes::TimestampMs;
    use crate::domain::string_newtypes::OutputText;
    use crate::domain::types::Role;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    state.push_user_input_line(OutputText::new("> user question"), TimestampMs::of(1));
    let mut assistant = crate::domain::tui_state::OutputLine::plain("assistant reply");
    assistant.header.timestamp = Some(TimestampMs::of(2));
    state.output.lines.push(assistant);
    let mut system = crate::domain::tui_state::OutputLine::plain("[system] system note");
    system.header.timestamp = Some(TimestampMs::of(3));
    state.output.lines.push(system);
    state.push_output_newline();
    state
        .output
        .lines
        .push(crate::domain::tui_state::OutputLine::tool_call(
            "tool output",
        ));
    state
        .output
        .lines
        .push(crate::domain::tui_state::OutputLine::error("error output"));
    state
        .output
        .lines
        .push(crate::domain::tui_state::OutputLine::self_feedback(
            "self feedback",
        ));

    let shift_tab = || KeyEvent {
        code: KeyCode::BackTab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };

    let _ = super::dispatch_chat_key(&mut state, shift_tab(), &handles).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let initial_snapshot = ask_handle.history_snapshot().await;
    assert_eq!(
        initial_snapshot.len(),
        3,
        "first ask open must restore only user/plain/system lines"
    );
    assert_eq!(initial_snapshot[0].role, Role::User);
    assert_eq!(initial_snapshot[0].content.as_str(), "user question");
    assert_eq!(initial_snapshot[1].role, Role::Assistant);
    assert_eq!(initial_snapshot[1].content.as_str(), "assistant reply");
    assert_eq!(initial_snapshot[2].role, Role::System);
    assert_eq!(initial_snapshot[2].content.as_str(), "[system] system note");

    let _ = super::dispatch_chat_key(&mut state, shift_tab(), &handles).await;
    let mut late_line = crate::domain::tui_state::OutputLine::plain("late main reply");
    late_line.header.timestamp = Some(TimestampMs::of(4));
    state.output.lines.push(late_line);
    let _ = super::dispatch_chat_key(&mut state, shift_tab(), &handles).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let reopened_snapshot = ask_handle.history_snapshot().await;
    assert_eq!(
        reopened_snapshot.len(),
        3,
        "reopening Ask must not restore main conversation a second time"
    );
    assert!(
        reopened_snapshot
            .iter()
            .all(|message| message.content.as_str() != "late main reply"),
        "messages added after first open must not be restored on reopen"
    );
}

/// Verifies that Enter with Ask focus routes only to the ask panel, echoes the
/// prompt into ask output, sets thinking, and does not submit to the main agent.
#[tokio::test]
async fn enter_with_ask_focus_submits_only_to_ask_panel() {
    use crate::domain::tui_state::{AskPanelState, InputFocus, SecondaryView};
    use crate::domain::types::Role;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Ask;
    state.prompt.buffer = "ask side question".to_owned();
    state.prompt.cursor = state.prompt.buffer.len();

    let enter = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let quit = super::dispatch_chat_key(&mut state, enter, &handles).await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "Ask submit must not quit the TUI"
    );
    assert!(
        provider.take_calls().is_empty(),
        "Ask-focused Enter must not submit to the main agent"
    );
    let panel = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("ask panel must remain present after submit");
    assert!(
        panel.thinking,
        "Ask submit must set ask_panel.thinking = true"
    );
    assert_eq!(panel.output[0].text.as_str(), "> ask side question");
    assert_eq!(panel.output[1].text.as_str(), "");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let ask_history = ask_handle.history_snapshot().await;
    assert!(
        ask_history.iter().any(|message| {
            message.role == Role::User && message.content.as_str() == "ask side question"
        }),
        "Ask-focused Enter must submit the prompt to the ask agent"
    );
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

/// Verifies that F10 in guided-plan mode routes to `force_advance()`.
#[tokio::test]
async fn guided_plan_f10_routes_to_force_advance() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let (guided_plan, mut cmd_rx) = make_guided_plan_command_handle();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "still typed".to_owned();

    let quit = super::dispatch_guided_plan_key(
        &mut state,
        KeyEvent {
            code: KeyCode::F(10),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        },
        &handles,
    )
    .await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "F10 must not quit the TUI"
    );
    match cmd_rx.recv().await {
        Some(crate::actors::guided_plan::commands::GuidedPlanCmd::ForceAdvance) => {}
        other => panic!("expected ForceAdvance command, got {other:?}"),
    }
    assert!(
        provider.take_calls().is_empty(),
        "F10 must not fall through to main chat submit"
    );
}

/// Verifies that Enter with an empty guided-plan buffer routes to `confirm_phase()`.
#[tokio::test]
async fn guided_plan_enter_with_empty_buffer_confirms_phase() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let (guided_plan, mut cmd_rx) = make_guided_plan_command_handle();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);

    let quit = super::dispatch_guided_plan_key(
        &mut state,
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        },
        &handles,
    )
    .await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "empty guided-plan Enter must not quit the TUI"
    );
    match cmd_rx.recv().await {
        Some(crate::actors::guided_plan::commands::GuidedPlanCmd::ConfirmPhase) => {}
        other => panic!("expected ConfirmPhase command, got {other:?}"),
    }
    assert!(
        provider.take_calls().is_empty(),
        "empty guided-plan Enter must not submit to the main agent"
    );
}

/// Verifies that Enter with a non-empty guided-plan buffer submits normal chat
/// text instead of confirming the phase.
#[tokio::test]
async fn guided_plan_enter_with_text_submits_normal_chat() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
    let (_, session) = crate::actors::session::session_actor::spawn(EndpointName::new("ep"));
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = crate::persistence::handle::PersistenceHandle::new(dir.path().to_owned());
    let (_scanner_join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let (guided_plan, mut cmd_rx) = make_guided_plan_command_handle();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "guided follow-up".to_owned();
    state.prompt.cursor = state.prompt.buffer.len();

    let quit = super::dispatch_guided_plan_key(
        &mut state,
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        },
        &handles,
    )
    .await;

    assert!(
        matches!(quit, std::ops::ControlFlow::Continue(())),
        "non-empty guided-plan Enter must not quit the TUI"
    );
    assert!(
        matches!(
            cmd_rx.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ),
        "non-empty guided-plan Enter must not send a guided-plan command"
    );
    let calls = provider.take_calls();
    assert_eq!(calls.len(), 1, "must submit exactly one main-agent turn");
    match &calls[0] {
        ProviderCall::Submit { prompt } => assert_eq!(prompt.as_str(), "guided follow-up"),
        other => panic!("expected plain Submit call, got {other:?}"),
    }
}

// --- Thinking mode picker integration tests ----------------------------------

/// Verifies that dispatching Down while the thinking mode picker is open does
/// NOT clear `pending_model_id`.
///
/// `dispatch_chat_key` calls `refresh_completion_hints` after every key.
/// When `pending_model_id` is set the buffer is empty (cleared after SelectModel
/// fired), so without the early-return guard the clear-all branch would wipe the
/// thinking mode state before `handle_submit` could read it.
#[tokio::test]
async fn dispatch_key_down_does_not_clear_thinking_mode_pending_model() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    // Simulate the state after a model was confirmed: buffer cleared, pending_model_id set.
    state.prompt.buffer = String::new();
    state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id = Some(crate::domain::string_newtypes::ModelId::new("gpt-5"));
    state.prompt.completions.model_picker.thinking_mode.selected = Some(0);

    let down = KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, down, &handles).await;

    assert!(
        state
            .prompt
            .completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .is_some(),
        "pending_model_id must NOT be cleared after a Down keypress in thinking mode"
    );
}

/// Verifies that dispatching Enter while the thinking mode picker is open calls
/// `set_model_with_options` with the selected `ReasoningEffort`.
///
/// When `pending_model_id` is set and a reasoning effort row is highlighted,
/// pressing Enter must invoke `set_model_with_options` on the provider with the
/// correct model id and effort level, then clear the thinking mode state.
#[tokio::test]
async fn dispatch_key_enter_confirms_thinking_mode_and_calls_set_model() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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

    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    // Simulate: model confirmed, thinking mode picker showing index 1 (High).
    state.prompt.buffer = String::new();
    state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id = Some(crate::domain::string_newtypes::ModelId::new("my-model"));
    // index 1 in ReasoningEffort::options() is High
    state.prompt.completions.model_picker.thinking_mode.selected = Some(1);

    let enter = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, enter, &handles).await;

    let calls = provider.take_set_model_options_calls();
    assert_eq!(
        calls.len(),
        1,
        "Enter in thinking mode must call set_model_with_options exactly once"
    );
    let (model_id, effort) = &calls[0];
    assert_eq!(model_id, "my-model", "must pass the pending model id");
    assert_eq!(
        effort.as_deref(),
        Some("high"),
        "selected index 1 must map to ReasoningEffort::High ('high')"
    );
    assert!(
        state
            .prompt
            .completions
            .model_picker
            .thinking_mode
            .pending_model_id
            .is_none(),
        "pending_model_id must be cleared after Enter confirms thinking mode"
    );
}

// ---------------------------------------------------------------------------
// Tab completion tests for command and file completions
// ---------------------------------------------------------------------------

/// Verifies that Tab when command completions are open applies the selected
/// command into the buffer and closes the completion menu.
#[tokio::test]
async fn tab_with_command_completions_applies_selected_command_and_closes_menu() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/qu".to_owned();
    state.prompt.completions.commands = vec![command_def("quit", "/quit", "Quit the TUI")];
    state.prompt.completions.command_selected = Some(0);

    let key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;

    assert_eq!(
        state.prompt.buffer, "/quit",
        "Tab must apply the selected command into the buffer"
    );
    assert!(
        state.prompt.completions.commands.is_empty(),
        "command completion list must be cleared after Tab"
    );
}

/// Verifies that Tab when command completions are open applies the first command
/// when no entry is explicitly selected.
#[tokio::test]
async fn tab_with_command_completions_defaults_to_first_when_none_selected() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "/qu".to_owned();
    state.prompt.completions.commands = vec![
        command_def("quit", "/quit", "Quit the TUI"),
        command_def("query", "/query <q>", "Query"),
    ];
    state.prompt.completions.command_selected = None;

    let key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;

    assert_eq!(
        state.prompt.buffer, "/quit",
        "Tab must apply the first command when none is selected"
    );
    assert!(state.prompt.completions.commands.is_empty());
}

/// Verifies that Tab when no completions are open does NOT apply a completion.
#[tokio::test]
async fn tab_with_no_completions_does_not_modify_buffer() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    let provider = RecordingChatProvider::new();
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
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "some text".to_owned();
    // No completions open

    let key = KeyEvent {
        code: KeyCode::Tab,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let _ = super::dispatch_chat_key(&mut state, key, &handles).await;

    assert_eq!(
        state.prompt.buffer, "some text",
        "Tab with no completions must not modify the buffer"
    );
}

/// Verifies that refresh_file_hints immediately returns directory contents when
/// the prefix ends with '/', using synchronous directory scan.
#[tokio::test]
async fn refresh_file_hints_immediately_expands_directory_on_slash_prefix() {
    let (join, scanner) = crate::actors::file_scanner::file_scanner_actor::spawn();
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    state.prompt.buffer = "@src/".to_owned();

    // No sleep - sync scan for slash-ending prefix must populate completions immediately
    super::refresh_file_hints(&mut state, &scanner);

    assert!(
        !state.prompt.completions.files.is_empty(),
        "file completions must be immediately populated for @src/ prefix without waiting for async scan"
    );
    scanner.shutdown();
    let _ = join.await;
}
