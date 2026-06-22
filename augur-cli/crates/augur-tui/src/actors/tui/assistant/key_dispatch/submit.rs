//! Prompt submission helpers for TUI key dispatch.

use crate::actors::tui::tui_actor::TuiHandles;
use crate::domain::tui_state::{
    current_timestamp_ms, AppState, ConversationMode, PendingResponseMeta,
};
use augur_core::actors::catalog_manager::models::OutputFormat;
use augur_core::actors::catalog_manager::models::ProviderName;
use augur_core::actors::deterministic_orchestrator::handle::PipelineResumeMode;
use augur_core::actors::file_scanner::parse_file_attachments;
use augur_core::config::provider_catalog::default_provider_catalog_dir;
use augur_core::domain::deterministic_orchestrator_ops::derive_feature_slug;
use augur_domain::domain::newtypes::{NumericNewtype, ScrollOffset, SupportsAuto};
use augur_domain::domain::string_newtypes::{
    FeatureContext, FeatureSlug, FilePath, ModelId, OutputText, PromptText, StringNewtype,
};
use augur_domain::domain::thinking_mode::ReasoningEffort;
use augur_domain::domain::types::CommandOutcome;

pub(crate) use super::completion::clear_all_completions;
use super::panel::open_ask_in_secondary;
use std::ops::ControlFlow;

struct CommandSubmission {
    text: PromptText,
    outcome: CommandOutcome,
}

struct SpecialAgentPrompt<'a> {
    status_label: &'a str,
    prompt: &'a str,
}

/// Execute a prompt submission: check for slash commands or dispatch to agent.
///
/// When the thinking mode picker is open (a model was already selected and Enter
/// was pressed again), this bypasses command parsing and calls
/// `handle_thinking_mode_confirm` instead.
pub(crate) async fn handle_submit(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    let thinking_mode_is_open = state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id
        .is_some();
    if thinking_mode_is_open {
        handle_thinking_mode_confirm(state, handles);
        return ControlFlow::Continue(());
    }
    let text = state.take_prompt();
    tracing::info!(
        prompt_len = text.as_str().chars().count(),
        "tui.submit.received"
    );
    clear_all_completions(state);
    let outcome = handles.tools.command.execute(&text);
    tracing::info!(
        outcome_kind = %command_outcome_kind(&outcome),
        "tui.submit.command_outcome"
    );
    if handle_command_outcome(state, handles, CommandSubmission { text, outcome }).await {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}

async fn handle_command_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    submission: CommandSubmission,
) -> bool {
    // Echo all slash commands to the conversation panel before any sub-handler runs.
    if submission.text.as_str().trim().starts_with('/')
        && !matches!(
            submission.outcome,
            CommandOutcome::NotACommand | CommandOutcome::Quit
        )
    {
        let ts = current_timestamp_ms();
        let raw = submission.text.as_str();
        state.push_user_input_line(OutputText::new(format!("> {raw}")), ts);
        handles
            .tools
            .logger
            .log_line(OutputText::from("user"), OutputText::from(raw));
        handles
            .persistence
            .queue_user_command(augur_domain::persistence::types::MessageRecord {
                message_type: augur_domain::persistence::types::MessageType::User,
                message: augur_domain::domain::types::Message {
                    role: augur_domain::domain::types::Role::User,
                    content: OutputText::new(raw),
                    timestamp: ts,
                    tool_call_id: None,
                    tool_calls: None,
                },
            });
        state.push_output_newline();
    }
    if let Some(should_quit) = handle_agent_control_outcome(state, handles, &submission) {
        return should_quit;
    }
    if let Some(should_quit) =
        handle_state_change_outcome(state, handles, &submission.outcome).await
    {
        return should_quit;
    }
    handle_submission_text_outcome(state, handles, submission)
}

fn submit_special_agent_prompt(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    prompt: SpecialAgentPrompt<'_>,
) {
    state.push_output_newline();
    start_pending_agent_response(state, prompt.status_label);
    let ep = state.agent.endpoint_name.clone();
    handles
        .agent
        .submit(PromptText::new(prompt.prompt), Some(ep));
}

fn submit_prompt_text(state: &mut AppState, handles: &TuiHandles<'_>, text: PromptText) {
    let (clean_text, attachments) = parse_file_attachments(&text);
    let is_empty = clean_text.as_str().trim().is_empty();
    if is_empty {
        return;
    }
    let previous_offset = state.output.scroll_offset.get();
    let had_selection = state.output.selection.is_some();
    state.output.scroll_offset.set(ScrollOffset::of(0));
    state.output.selection = None;
    tracing::info!(
        previous_offset = previous_offset.inner(),
        new_offset = 0,
        had_selection,
        "tui.submit.main_route.scroll_reset"
    );
    let ts = current_timestamp_ms();
    tracing::info!(
        prompt_len = text.as_str().chars().count(),
        has_attachments = !attachments.is_empty(),
        "tui.submit.main_route.user_line"
    );
    state.push_user_input_line(OutputText::new(format!("> {}", text.as_str())), ts);
    handles
        .tools
        .logger
        .log_line(OutputText::from("user"), OutputText::from(text.as_str()));
    state.push_output_newline();
    state.push_output_newline();
    start_pending_agent_response(state, "Thinking...");
    let ep = state.agent.endpoint_name.clone();
    tracing::info!(
        endpoint = %ep,
        has_attachments = !attachments.is_empty(),
        "tui.submit.main_route.dispatch_agent"
    );
    if attachments.is_empty() {
        handles.agent.submit(text, Some(ep));
    } else {
        handles
            .agent
            .submit_with_attachments(clean_text, Some(ep), attachments);
    }
}

fn run_guided_plan(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    path: augur_domain::domain::string_newtypes::FilePath,
) {
    match augur_core::actors::guided_plan::loader::load_guided_plan(std::path::Path::new(
        path.as_str(),
    )) {
        Ok(config) => {
            let ui_state = crate::domain::tui_state::GuidedPlanUiState::from_config(&config);
            state.interaction.mode = ConversationMode::GuidedPlan(ui_state);
            handles.tools.guided_plan.start(config, path.clone());
            push_system_line(state, format!("[system] guided plan started: {}", path));
        }
        Err(e) => {
            state.push_error_line(format!("[error] /run-plan: {e}"));
            state.push_output_newline();
        }
    }
}

fn start_pending_agent_response(state: &mut AppState, status_label: &str) {
    state.agent.thinking.is_active = true.into();
    state.agent.thinking.label = status_label.into();
    state.agent.pending_response = Some(
        PendingResponseMeta::builder()
            .ts(current_timestamp_ms())
            .model(state.status.model_display.clone())
            .build(),
    );
}

fn push_system_line(state: &mut AppState, message: impl Into<OutputText>) {
    state.push_system_message(message);
    state.push_output_newline();
}

fn handle_agent_control_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    submission: &CommandSubmission,
) -> Option<bool> {
    if let Some(result) = handle_agent_control_simple_outcome(state, handles, &submission.outcome) {
        return Some(result);
    }
    handle_agent_control_workflow_outcome(state, handles, submission)
}

fn handle_agent_control_simple_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    handle_agent_control_core_outcome(state, handles, outcome)
        .or_else(|| handle_agent_control_prompt_outcome(state, handles, outcome))
}

fn handle_agent_control_core_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    match outcome {
        CommandOutcome::Quit => Some(true),
        CommandOutcome::CompactSession => {
            handles.agent.compact();
            Some(false)
        }
        CommandOutcome::StopExecution => {
            state.push_system_message(OutputText::new(
                "[system] stopping current execution...".to_owned(),
            ));
            state.push_output_newline();
            handles.agent.interrupt();
            Some(false)
        }
        _ => None,
    }
}

fn handle_agent_control_prompt_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    match outcome {
        CommandOutcome::CommitChanges => {
            submit_special_agent_prompt(state, handles, commit_prompt());
            Some(false)
        }
        CommandOutcome::PushBranch => {
            submit_special_agent_prompt(state, handles, push_prompt());
            Some(false)
        }
        CommandOutcome::OpenAskPanel => {
            open_ask_in_secondary(state, handles);
            Some(false)
        }
        _ => None,
    }
}

fn handle_agent_control_workflow_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    submission: &CommandSubmission,
) -> Option<bool> {
    match &submission.outcome {
        CommandOutcome::RunBackgroundAgent { agent, prompt } => {
            tracing::info!(
                agent = %agent,
                prompt_len = prompt.as_str().chars().count(),
                "tui.submit.background_agent.dispatch"
            );
            handles
                .agent
                .run_background_agent(agent.clone(), prompt.clone());
            Some(false)
        }
        CommandOutcome::StartPipeline { resume } => {
            start_pipeline(
                state,
                handles,
                PipelineStartArgs {
                    text: submission.text.clone(),
                    resume: *resume,
                },
            );
            Some(false)
        }
        _ => None,
    }
}

fn command_outcome_kind(outcome: &CommandOutcome) -> &'static str {
    if let Some(kind) = command_outcome_kind_control(outcome) {
        return kind;
    }
    if let Some(kind) = command_outcome_kind_selection(outcome) {
        return kind;
    }
    command_outcome_kind_workflow(outcome)
}

fn command_outcome_kind_control_meta(outcome: &CommandOutcome) -> Option<&'static str> {
    resolve_command_outcome_kind(outcome, CONTROL_META_OUTCOME_CASES)
}

fn command_outcome_kind_control_action(outcome: &CommandOutcome) -> Option<&'static str> {
    resolve_command_outcome_kind(outcome, CONTROL_ACTION_OUTCOME_CASES)
}

fn command_outcome_kind_control(outcome: &CommandOutcome) -> Option<&'static str> {
    command_outcome_kind_control_meta(outcome)
        .or_else(|| command_outcome_kind_control_action(outcome))
}

fn command_outcome_kind_selection(outcome: &CommandOutcome) -> Option<&'static str> {
    resolve_command_outcome_kind(outcome, SELECTION_OUTCOME_CASES)
}

type OutcomeKindPredicate = fn(&CommandOutcome) -> bool;

struct OutcomeKindCase {
    predicate: OutcomeKindPredicate,
    label: &'static str,
}

const CONTROL_META_OUTCOME_CASES: &[OutcomeKindCase] = &[
    OutcomeKindCase {
        predicate: is_quit,
        label: "Quit",
    },
    OutcomeKindCase {
        predicate: is_switch_endpoint,
        label: "SwitchEndpoint",
    },
    OutcomeKindCase {
        predicate: is_system_message,
        label: "SystemMessage",
    },
    OutcomeKindCase {
        predicate: is_not_a_command,
        label: "NotACommand",
    },
    OutcomeKindCase {
        predicate: is_unknown_command,
        label: "UnknownCommand",
    },
];

const CONTROL_ACTION_OUTCOME_CASES: &[OutcomeKindCase] = &[
    OutcomeKindCase {
        predicate: is_compact_session,
        label: "CompactSession",
    },
    OutcomeKindCase {
        predicate: is_stop_execution,
        label: "StopExecution",
    },
    OutcomeKindCase {
        predicate: is_commit_changes,
        label: "CommitChanges",
    },
    OutcomeKindCase {
        predicate: is_push_branch,
        label: "PushBranch",
    },
];

const SELECTION_OUTCOME_CASES: &[OutcomeKindCase] = &[
    OutcomeKindCase {
        predicate: is_select_model,
        label: "SelectModel",
    },
    OutcomeKindCase {
        predicate: is_select_auto_model,
        label: "SelectAutoModel",
    },
    OutcomeKindCase {
        predicate: is_run_plan,
        label: "RunPlan",
    },
    OutcomeKindCase {
        predicate: is_new_session,
        label: "NewSession",
    },
    OutcomeKindCase {
        predicate: is_open_ask_panel,
        label: "OpenAskPanel",
    },
];

fn resolve_command_outcome_kind(
    outcome: &CommandOutcome,
    cases: &[OutcomeKindCase],
) -> Option<&'static str> {
    cases
        .iter()
        .find_map(|case| (case.predicate)(outcome).then_some(case.label))
}

fn is_quit(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::Quit)
}

fn is_switch_endpoint(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::SwitchEndpoint(_))
}

fn is_system_message(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::SystemMessage(_))
}

fn is_not_a_command(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::NotACommand)
}

fn is_unknown_command(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::UnknownCommand)
}

fn is_compact_session(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::CompactSession)
}

fn is_stop_execution(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::StopExecution)
}

fn is_commit_changes(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::CommitChanges)
}

fn is_push_branch(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::PushBranch)
}

fn is_select_model(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::SelectModel(_))
}

fn is_select_auto_model(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::SelectAutoModel)
}

fn is_run_plan(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::RunPlan(_))
}

fn is_new_session(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::NewSession)
}

fn is_open_ask_panel(outcome: &CommandOutcome) -> bool {
    matches!(outcome, CommandOutcome::OpenAskPanel)
}

fn command_outcome_kind_workflow(outcome: &CommandOutcome) -> &'static str {
    match outcome {
        CommandOutcome::RunBackgroundAgent { .. } => "RunBackgroundAgent",
        CommandOutcome::StartPipeline { .. } => "StartPipeline",
        CommandOutcome::GenerateCatalog { .. } => "GenerateCatalog",
        CommandOutcome::Quit
        | CommandOutcome::SwitchEndpoint(_)
        | CommandOutcome::SystemMessage(_)
        | CommandOutcome::NotACommand
        | CommandOutcome::UnknownCommand
        | CommandOutcome::CompactSession
        | CommandOutcome::StopExecution
        | CommandOutcome::CommitChanges
        | CommandOutcome::PushBranch
        | CommandOutcome::SelectModel(_)
        | CommandOutcome::SelectAutoModel
        | CommandOutcome::RunPlan(_)
        | CommandOutcome::NewSession
        | CommandOutcome::OpenAskPanel => unreachable!("covered by command_outcome_kind helpers"),
    }
}

/// Parse a `--resume` flag from `text`.
///
/// Returns `(true, remainder)` when `--resume` is present; `(false, text.to_owned())` otherwise.
/// The remainder has `--resume` removed and excess whitespace collapsed.
fn parse_resume_flag(text: &str) -> (bool, String) {
    let mut words: Vec<&str> = text.split_whitespace().collect();
    if let Some(pos) = words.iter().position(|w| *w == "--resume") {
        words.remove(pos);
        return (true, words.join(" "));
    }
    (false, text.to_owned())
}

/// Parse a `--slug <value>` flag from `text`.
///
/// Returns `(Some(slug), remainder)` when the flag is present; `(None, text.to_owned())` otherwise.
/// The remainder has `--slug <value>` removed and excess whitespace collapsed.
fn parse_slug_flag(text: &str) -> (Option<String>, String) {
    let mut words: Vec<&str> = text.split_whitespace().collect();
    if let Some(pos) = words.iter().position(|w| *w == "--slug")
        && pos + 1 < words.len()
    {
        let slug = words[pos + 1].to_owned();
        words.remove(pos + 1);
        words.remove(pos);
        return (Some(slug), words.join(" "));
    }
    (None, text.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Arguments for starting the deterministic orchestrator pipeline.
///
/// Bundles the user-supplied text and resume flag so `start_pipeline` stays
/// within the three-parameter limit.
struct PipelineStartArgs {
    /// Full prompt text from the user's input, including the `/run-pipeline` prefix.
    text: PromptText,
    /// When `true`, the orchestrator skips steps whose output artifacts already exist.
    resume: bool,
}

struct RefreshEndpointCatalogArgs<'a> {
    config: &'a augur_domain::config::types::AppConfig,
    provider_dir: &'a std::path::Path,
}

/// Starts the deterministic orchestrator pipeline with the given prompt text as feature context.
///
/// Inputs:
/// - `state`: mutable app state (used to push system status messages).
/// - `handles`: TUI handles providing access to the orchestrator.
/// - `args`: bundled pipeline start arguments (text and resume flag).
///
/// Side effects:
/// - Pushes a system message confirming pipeline start.
/// - Sends `Start` command to the deterministic orchestrator.
///
/// The slug is always derived from the command text so it is never carried over
/// from a previous orchestrator run. An explicit `--slug <value>` flag takes
/// priority; otherwise the slug is derived from the feature request text. If
/// neither is present, `None` is passed and the orchestrator starts with no slug.
fn start_pipeline(state: &mut AppState, handles: &TuiHandles<'_>, args: PipelineStartArgs) {
    let PipelineStartArgs { text, resume } = args;
    let parsed = parse_pipeline_start_input(text.as_str());
    let feature_slug = derive_pipeline_feature_slug(&parsed);
    let feature_context = build_pipeline_feature_context(parsed.clean_text, parsed.attachments);
    let status_msg = match &feature_slug {
        Some(slug) => format!("[system] starting pipeline (slug: {slug})..."),
        None => "[system] starting pipeline...".to_owned(),
    };
    push_system_line(state, status_msg);
    let mapped_feature_context = feature_context.map(FeatureContext::from);
    handles.work.orchestrator.start(
        mapped_feature_context,
        feature_slug,
        if resume {
            PipelineResumeMode::ResumeExisting
        } else {
            PipelineResumeMode::StartFresh
        },
    );
}

struct ParsedPipelineStart {
    slug_source: String,
    explicit_slug: Option<String>,
    clean_text: PromptText,
    attachments: Vec<FilePath>,
}

fn parse_pipeline_start_input(raw: &str) -> ParsedPipelineStart {
    let stripped = raw.strip_prefix("/run-pipeline").unwrap_or(raw).trim();
    let (_resume_flag, resume_stripped) = parse_resume_flag(stripped);
    let (explicit_slug, slug_stripped) = parse_slug_flag(&resume_stripped);
    let slug_stripped_prompt = PromptText::new(slug_stripped.clone());
    let (clean_text, attachments) = parse_file_attachments(&slug_stripped_prompt);
    ParsedPipelineStart {
        slug_source: slug_stripped,
        explicit_slug,
        clean_text,
        attachments,
    }
}

fn derive_pipeline_feature_slug(parsed: &ParsedPipelineStart) -> Option<FeatureSlug> {
    parsed
        .explicit_slug
        .clone()
        .map(FeatureSlug::from)
        .or_else(|| {
            let source = parsed.slug_source.trim();
            (!source.is_empty())
                .then(|| derive_feature_slug(&FeatureContext::from(source.to_owned())))
        })
}

fn build_pipeline_feature_context(
    clean_text: PromptText,
    attachments: Vec<FilePath>,
) -> Option<String> {
    if clean_text.as_str().trim().is_empty() && attachments.is_empty() {
        return None;
    }
    let mut context = clean_text.as_str().to_owned();
    for path in &attachments {
        if let Ok(content) = std::fs::read_to_string(path.as_str()) {
            context.push('\n');
            context.push_str(&content);
        }
    }
    Some(context)
}

async fn handle_state_change_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    if let Some(result) = handle_state_change_sync_outcome(state, handles, outcome) {
        return Some(result);
    }
    handle_state_change_async_outcome(state, handles, outcome).await
}

fn handle_state_change_sync_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    handle_state_change_message_or_model(state, handles, outcome)
        .or_else(|| handle_state_change_session(state, handles, outcome))
}

fn handle_state_change_message_or_model(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    match outcome {
        CommandOutcome::SystemMessage(msg) => {
            state.push_system_message(msg.clone());
            state.push_output_newline();
            state.push_output_newline();
            Some(false)
        }
        CommandOutcome::SelectModel(model_id) => {
            Some(handle_select_model(state, handles, model_id))
        }
        CommandOutcome::SelectAutoModel => Some(handle_select_auto_model(state, handles)),
        _ => None,
    }
}

fn handle_state_change_session(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    match outcome {
        CommandOutcome::RunPlan(path) => Some(handle_run_plan(state, handles, path.clone())),
        CommandOutcome::NewSession => Some(handle_new_session(state, handles)),
        _ => None,
    }
}

async fn handle_state_change_async_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    outcome: &CommandOutcome,
) -> Option<bool> {
    match outcome {
        CommandOutcome::SwitchEndpoint(name) => {
            Some(handle_switch_endpoint(state, handles, name).await)
        }
        CommandOutcome::GenerateCatalog { provider } => {
            Some(handle_generate_catalog(state, handles, provider).await)
        }
        _ => None,
    }
}

async fn handle_switch_endpoint(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    name: &augur_domain::domain::string_newtypes::EndpointName,
) -> bool {
    if handles.session.set_endpoint(name.clone()).await.is_ok() {
        apply_switch_model_state(state, handles, name);
        push_system_line(
            state,
            format!("[system] switched to endpoint: {}", name.as_str()),
        );
        // Save user settings: clear model on endpoint switch (model resets to default)
        handles
            .session
            .save_user_settings(Some(name), None::<&ModelId>, None::<&ReasoningEffort>);
    } else {
        push_system_line(
            state,
            format!(
                "[system] failed to switch endpoint: {} (session queue unavailable)",
                name.as_str()
            ),
        );
    }
    false
}

fn handle_select_model(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    model_id: &augur_domain::domain::string_newtypes::ModelId,
) -> bool {
    let active_endpoint = handles.session.active_endpoint();
    let is_known_model = state
        .prompt
        .models
        .available
        .iter()
        .any(|m| &m.id == model_id);
    if !is_known_model {
        push_system_line(
            state,
            format!(
                "[system] model '{}' is not available for endpoint '{}'",
                model_id.as_str(),
                active_endpoint.as_str()
            ),
        );
        return false;
    }
    state
        .prompt
        .completions
        .model_picker
        .open_thinking_mode(model_id.clone());
    false
}

fn handle_select_auto_model(state: &mut AppState, handles: &TuiHandles<'_>) -> bool {
    let active_endpoint = handles.session.active_endpoint();
    if !endpoint_supports_auto(state, &active_endpoint) {
        push_system_line(
            state,
            format!(
                "[system] auto model selection is not supported for endpoint '{}'",
                active_endpoint.as_str()
            ),
        );
        return false;
    }
    handles.agent.set_model(ModelId::new(""));
    state.prompt.models.active_id = Some(ModelId::new(""));
    state.status.model_display = "auto".into();
    push_system_line(state, "[system] model: auto");
    // Save auto model selection
    handles.session.save_user_settings(
        Some(&active_endpoint),
        None::<&ModelId>, // auto = no model override
        None::<&ReasoningEffort>,
    );
    false
}

fn handle_run_plan(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    path: augur_domain::domain::string_newtypes::FilePath,
) -> bool {
    run_guided_plan(state, handles, path);
    false
}

fn handle_new_session(state: &mut AppState, handles: &TuiHandles<'_>) -> bool {
    let active_endpoint = handles.session.active_endpoint();
    state.agent.endpoint_name = active_endpoint.clone();
    handles.persistence.reset_to_new_session();
    handles.agent.replace_session(None);
    tracing::info!(endpoint = %active_endpoint, "tui.new_session.reset_provider_session");
    state.reset_for_new_session();
    push_system_line(state, "[system] new session started");
    false
}

async fn handle_generate_catalog(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    provider: &Option<String>,
) -> bool {
    push_system_line(state, "[system] generating catalog...");
    let provider_filter = provider.as_ref().map(|name| ProviderName(name.clone()));
    match handles
        .work
        .catalog_manager
        .generate_catalog(provider_filter, OutputFormat::Yaml)
        .await
    {
        Ok(output) => {
            state.push_system_message(output);
            state.push_output_newline();
            match load_active_runtime_config() {
                Ok(config) => {
                    let provider_dir = default_provider_catalog_dir();
                    refresh_endpoint_catalog_from_provider_dir(
                        state,
                        handles,
                        RefreshEndpointCatalogArgs {
                            config: &config,
                            provider_dir: provider_dir.as_path(),
                        },
                    );
                    push_system_line(
                        state,
                        "[system] refreshed model catalog from configs/providers",
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "catalog refresh failed after write");
                    push_system_line(state, "[system] catalog written, but model refresh failed")
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "catalog generation failed");
            push_system_line(
                state,
                "[system] catalog generation failed: unable to fetch and write provider catalog data",
            );
        }
    }
    false
}

fn load_active_runtime_config() -> anyhow::Result<augur_domain::config::types::AppConfig> {
    if let Ok(path) = std::env::var("AUGUR_CLI_CONFIG_PATH") {
        let file_path = FilePath::new(path.as_str());
        return augur_core::config::load_config(Some(&file_path));
    }
    augur_core::config::load_config(None)
}

fn refresh_endpoint_catalog_from_provider_dir(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    args: RefreshEndpointCatalogArgs<'_>,
) {
    let RefreshEndpointCatalogArgs {
        config,
        provider_dir,
    } = args;
    state.prompt.models.endpoint_catalog =
        discover_runtime_endpoint_catalog_for_provider_dir(config, provider_dir);
    let active_endpoint = handles.session.active_endpoint();
    apply_switch_model_state(state, handles, &active_endpoint);
}

fn discover_runtime_endpoint_catalog_for_provider_dir(
    config: &augur_domain::config::types::AppConfig,
    provider_dir: &std::path::Path,
) -> Vec<crate::domain::tui_state::EndpointModelCatalog> {
    augur_core::config::endpoint_catalog_discovery::discover_endpoint_catalog_for_provider_dir(
        config,
        provider_dir,
    )
}

fn apply_switch_model_state(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    endpoint: &augur_domain::domain::string_newtypes::EndpointName,
) {
    let catalog_row = state
        .prompt
        .models
        .endpoint_catalog
        .iter()
        .find(|row| &row.endpoint_name == endpoint);
    match catalog_row {
        Some(row) => {
            state.prompt.models.available = row.models.clone();
            state.status.model_display = row.default_display.clone();
            state.prompt.models.active_id = if row.supports_auto.into() {
                let auto_id = ModelId::new("");
                handles.agent.set_model(auto_id.clone());
                state.status.model_display = "auto".into();
                Some(auto_id)
            } else {
                None
            };
        }

        None => {
            state.prompt.models.available.clear();
            state.prompt.models.active_id = None;
            state.status.model_display = endpoint.as_str().into();
        }
    }
    state.prompt.completions.model_picker.items.clear();
    state.prompt.completions.model_picker.selected = None;
    state.prompt.completions.model_picker.thinking_mode =
        crate::domain::tui_state::ThinkingModeCompletion::default();
}

fn endpoint_supports_auto(
    state: &AppState,
    endpoint: &augur_domain::domain::string_newtypes::EndpointName,
) -> bool {
    state
        .prompt
        .models
        .endpoint_catalog
        .iter()
        .find(|row| &row.endpoint_name == endpoint)
        .map(|row| row.supports_auto)
        .unwrap_or(SupportsAuto::no())
        .into()
}

fn handle_submission_text_outcome(
    state: &mut AppState,
    handles: &TuiHandles<'_>,
    submission: CommandSubmission,
) -> bool {
    let CommandSubmission { text, outcome } = submission;
    match outcome {
        CommandOutcome::UnknownCommand => {
            push_system_line(
                state,
                format!("[system] unknown command: {}", text.as_str()),
            );
            false
        }
        CommandOutcome::NotACommand => {
            submit_prompt_text(state, handles, text);
            false
        }
        _ => false,
    }
}

fn commit_prompt() -> SpecialAgentPrompt<'static> {
    SpecialAgentPrompt {
        status_label: "Committing...",
        prompt: "create message and commit",
    }
}

fn push_prompt() -> SpecialAgentPrompt<'static> {
    SpecialAgentPrompt {
        status_label: "Pushing...",
        prompt: "push commits to remote origin",
    }
}

/// Confirm the thinking mode picker: read the selected reasoning effort, call
/// `set_model_with_options`, and clear the thinking mode state.
///
/// Called by `handle_submit` when `thinking_mode.pending_model_id` is `Some`.
/// The `selected` index maps into `ReasoningEffort::options()`. When `None`,
/// defaults to `ReasoningEffort::Auto`.
pub(crate) fn handle_thinking_mode_confirm(state: &mut AppState, handles: &TuiHandles<'_>) {
    let options = ReasoningEffort::options();
    let selected_idx = state.prompt.completions.model_picker.thinking_mode.selected;
    let effort = selected_idx
        .and_then(|i| options.get(i).copied())
        .unwrap_or(ReasoningEffort::Auto);
    let model_id = state
        .prompt
        .completions
        .model_picker
        .thinking_mode
        .pending_model_id
        .take()
        .unwrap_or_else(|| ModelId::new(""));
    state.prompt.completions.model_picker.thinking_mode.selected = None;
    let active_endpoint = handles.session.active_endpoint();
    let model_for_save: Option<ModelId> = if model_id.as_str().is_empty() {
        None
    } else {
        Some(model_id.clone())
    };
    handles.agent.set_model_with_options(model_id, Some(effort));
    // Save user settings when model is confirmed
    handles.session.save_user_settings(
        Some(&active_endpoint),
        model_for_save.as_ref(),
        Some(&effort),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::config::types::{
        AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials,
        PersistenceConfig, Provider,
    };
    use augur_domain::domain::newtypes::{NumericNewtype, Temperature};
    use augur_domain::domain::string_newtypes::{
        EndpointName, EndpointUrl, FilePath, ModelName, OutputText, StringNewtype,
    };
    use augur_domain::domain::TokenCount;

    fn test_config() -> AppConfig {
        AppConfig {
            endpoints: vec![EndpointConfig {
                name: EndpointName::new("primary"),
                provider: Provider::OpenAi,
                base_url: EndpointUrl::new("https://api.openai.com/v1"),
                model: ModelName::new("fallback-model"),
                credentials: EndpointCredentials::default(),
            }],
            default_endpoint: EndpointName::new("primary"),
            agent: AgentConfig {
                system_prompt: OutputText::new("test"),
                max_tokens: TokenCount::new(256),
                temperature: Temperature::new(0.2),
                allowed_dirs: vec![FilePath::new("./")],
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

    #[test]
    fn submit_runtime_catalog_discovery_uses_provider_catalog_models() {
        let provider_dir = tempfile::tempdir().expect("provider tempdir");
        std::fs::write(
            provider_dir.path().join("openai.yaml"),
            r#"provider: openai
models:
  - id: gpt-replacement
    display_name: GPT Replacement
    cost_input_per_mtok: 1.0
    cost_output_per_mtok: 2.0
"#,
        )
        .expect("write provider catalog");
        let rows =
            discover_runtime_endpoint_catalog_for_provider_dir(&test_config(), provider_dir.path());
        let row = rows
            .iter()
            .find(|row| row.endpoint_name == EndpointName::new("primary"))
            .expect("primary endpoint row");
        assert_eq!(row.models.len(), 1);
        assert_eq!(row.models[0].id.as_str(), "gpt-replacement");
        assert_ne!(row.models[0].id.as_str(), "fallback-model");
    }
}
