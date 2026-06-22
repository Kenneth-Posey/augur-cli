//! Actor wiring: constructs all actors and connects them via channels.
//!
//! [`run`] is the single entry point called by `main`. It spawns actors in
//! dependency order, waits for the TUI to signal shutdown, then shuts
//! down all other actors before returning.

use augur_core::actors;
use augur_core::actors::LlmFeedConsumerHandle;
use augur_core::actors::UserMessageConsumerHandle;
use augur_core::actors::cache::handle::CacheHandle;
use augur_core::actors::file_read::FileReadHandle;
use augur_core::actors::history_adapter::handle::HistoryAdapterHandle;
use augur_domain::config::types::AppConfig;
use augur_domain::config::types::ProgramSettings;
use augur_domain::domain::newtypes::TimestampSecs;
use augur_domain::domain::traits::ChatProvider;
use augur_domain::domain::types::{AgentOutput, FeedEntry};
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_tui::actors::tui::tui_actor::TuiSubActorHandles;
use augur_tui::domain::tui_render::AppRenderer;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Default directory to watch for file changes in the cache actor.
/// This is the root source directory of the project under analysis.
pub const DEFAULT_CACHE_WATCH_DIR: &str = "src";

type TaskJoin = tokio::task::JoinHandle<()>;
type AgentOutputReceiver =
    tokio::sync::broadcast::Receiver<augur_domain::domain::types::AgentOutput>;
type SupervisorReceiver =
    tokio::sync::broadcast::Receiver<augur_domain::domain::types::SupervisorEvent>;

/// Runtime configuration bundle for wiring entrypoints.
pub struct RunConfig {
    pub config: AppConfig,
    pub program_settings: ProgramSettings,
}

/// Bundles the command handles for the two feed-consumer actors.
///
/// Both handles must be kept alive for the lifetime of the application so the
/// consumer tasks do not exit prematurely. Dropping either handle closes that
/// actor's command channel, causing it to exit and silently discard all
/// subsequent output. Stored in `\`OptionalHandles\`` and dropped via
/// `shutdown_runtime`.
pub struct ConsumerHandles {
    pub llm_feed: LlmFeedConsumerHandle,
    pub user_message: UserMessageConsumerHandle,
}

struct QueryChannels {
    _tx: mpsc::Sender<QueryUserRequest>,
    rx: Option<mpsc::Receiver<QueryUserRequest>>,
}

/// Join handles for the three primary actor tasks (LLM, file-read, tool).
#[derive(bon::Builder)]
struct CoreActorJoins {
    llm: TaskJoin,
    file_read: TaskJoin,
    tool: TaskJoin,
}

/// Join handles for the three support actor tasks (logger, token-tracker, LSP).
#[derive(bon::Builder)]
struct CoreSupportJoins {
    logger: TaskJoin,
    token_tracker: TaskJoin,
    lsp: TaskJoin,
}

struct CoreIoHandles {
    logger: actors::LoggerHandle,
    history_adapter: HistoryAdapterHandle,
}

struct CoreServiceHandles {
    llm: augur_provider_openrouter::actors::LlmHandle,
    file_read: FileReadHandle,
    tool: actors::ToolHandle,
}

struct CoreHandles {
    services: CoreServiceHandles,
    cache: Option<CacheHandle>,
    catalog_manager: augur_core::actors::catalog_manager::CatalogManagerHandle,
    io: CoreIoHandles,
}

struct CoreStartup {
    persistence: PersistenceHandle,
    sessions_dir: std::path::PathBuf,
    session_summaries: Vec<augur_domain::persistence::types::SessionSummary>,
    token_tracker: actors::TokenTrackerHandle,
}

/// Runtime bundle for core infrastructure actors and startup resources.
///
/// Holds core actor joins/handles, query channels, startup persistence/session
/// context, and the command handle needed to drive shutdown and orchestration.
pub struct CoreRuntime {
    actor_joins: CoreActorJoins,
    support_joins: CoreSupportJoins,
    handles: CoreHandles,
    context: CoreRuntimeContext,
}

struct CoreRuntimeContext {
    query: QueryChannels,
    startup: CoreStartup,
    control: CoreControl,
}

struct CoreControl {
    command: actors::CommandHandle,
    /// Shared agent-output broadcast sender.
    ///
    /// Created in `spawn_core_runtime` and passed to the LLM actor at startup
    /// so it can emit `ModelsAvailable`. The main agent actor is spawned with
    /// this same sender so all subscribers see a unified output stream.
    agent_tx: tokio::sync::broadcast::Sender<AgentOutput>,
    /// OpenRouter orchestrator handle used for provider-aware session/reset semantics.
    openrouter_orchestrator_handle:
        augur_provider_openrouter::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle,
    /// Active-model handle paired with the OpenRouter orchestrator runtime.
    openrouter_active_model_handle: actors::ActiveModelHandle,
    /// Receiver for OpenRouter task feed output; forwarded into the app feed path.
    openrouter_feed_rx: Option<mpsc::Receiver<FeedEntry>>,/// Clone of the LSP handle retained for deterministic kill-on-shutdown.
    ///
    /// The original handle is consumed by the tool registry at startup. This
    /// clone is kept separately so `shutdown_runtime` can call `kill()` to
    /// terminate the rust-analyzer child process before awaiting the join
    /// handle, preventing orphaned processes.
    lsp_handle: augur_core::actors::lsp::LspHandle,
}

/// Test-visible Ask actor runtime bundle.
pub struct AskRuntime {
    pub join: TaskJoin,
    pub tool_join: Option<TaskJoin>,
    pub handle: actors::AskHandle,
}

pub struct ChatRuntimeInput {
    agent_handle: actors::AgentHandle,
    session_handle: actors::SessionHandle,
    agent_feed_tx: mpsc::Sender<FeedEntry>,
}

pub struct ChatRuntime {
    provider: Arc<dyn ChatProvider>,
    output_rx: AgentOutputReceiver,
    join: Option<TaskJoin>,
}

pub struct SupervisorRuntime {
    rx: Option<SupervisorReceiver>,
    join: Option<TaskJoin>,
    handle: Option<actors::SupervisorHandle>,
}

struct TuiProviders {
    chat: Arc<dyn ChatProvider>,
    session: actors::SessionHandle,
    tools: augur_tui::actors::tui::tui_actor::TuiServiceTools,
    orchestrator: actors::DeterministicOrchestratorHandle,
}

struct TuiChannels {
    output: AgentOutputReceiver,
    query: mpsc::Receiver<QueryUserRequest>,
    supervisor: Option<SupervisorReceiver>,
    feed_tx: mpsc::Sender<FeedEntry>,
    feed_rx: mpsc::Receiver<FeedEntry>,
}

pub struct TuiRuntimeInput {
    config: AppConfig,
    renderer: AppRenderer,
    providers: TuiProviders,
    channels: TuiChannels,
    sub_actors: TuiSubActorHandles,
}

struct AppJoins {
    primary: PrimaryJoins,
    optional: OptionalJoins,
}

struct PrimaryJoins {
    domain: PrimaryDomainJoins,
    ui: PrimaryUiJoins,
}

struct PrimaryDomainJoins {
    agent: TaskJoin,
    session: TaskJoin,
    ask_agent: TaskJoin,
    deterministic_orchestrator: TaskJoin,
    file_scanner: TaskJoin,
}

struct PrimaryUiJoins {
    tui: TaskJoin,
}

struct OptionalJoins {
    ask_tool: Option<TaskJoin>,
    copilot: Option<TaskJoin>,
    executor: Option<TaskJoin>,
}

struct AppHandles {
    primary: PrimaryHandles,
    optional: OptionalHandles,
}

struct PrimaryHandles {
    domain: PrimaryDomainHandles,
    ui: PrimaryUiHandles,
}

struct PrimaryDomainHandles {
    agent: actors::AgentHandle,
    session: actors::SessionHandle,
    file_scanner: actors::FileScannerHandle,
    guided_plan: actors::GuidedPlanHandle,
    deterministic_orchestrator: actors::DeterministicOrchestratorHandle,
}

struct PrimaryUiHandles {
    tui: augur_tui::TuiHandle,
}

struct OptionalHandles {
    ask_shutdown: actors::AskHandle,
    chat_provider: Arc<dyn ChatProvider>,
    supervisor: Option<actors::SupervisorHandle>,
    consumers: ConsumerHandles,
}

struct AppRuntime {
    joins: AppJoins,
    handles: AppHandles,
}

/// Top-level runtime bundle returned by wiring bootstrap.
///
/// Combines the core runtime and application runtime so callers can manage
/// lifecycle and shutdown across all spawned actors.
pub struct RunRuntime {
    core: CoreRuntime,
    app: AppRuntime,
}

/// Test-visible wrapper for actor handle + join handle pairs.
pub struct ActorRuntime<H> {
    pub join: TaskJoin,
    pub handle: H,
}

/// Test-visible bundle of all spawned application actors.
pub struct SpawnedAppActors {
    pub domain: SpawnedDomainActors,
    pub planning: SpawnedPlanningActors,
    pub ui: SpawnedUiActors,
    pub optional: SpawnedOptionalActors,
}

/// Test-visible bundle of domain layer actors.
pub struct SpawnedDomainActors {
    pub agent: ActorRuntime<actors::AgentHandle>,
    pub session: ActorRuntime<actors::SessionHandle>,
    pub ask: AskRuntime,
    pub deterministic_orchestrator: ActorRuntime<actors::DeterministicOrchestratorHandle>,
}

/// Test-visible bundle of planning layer actors.
pub struct SpawnedPlanningActors {
    pub file_scanner: ActorRuntime<actors::FileScannerHandle>,
    pub guided_plan: actors::GuidedPlanHandle,
}

/// Test-visible bundle of UI layer actors.
pub struct SpawnedUiActors {
    pub tui: ActorRuntime<augur_tui::TuiHandle>,
}

pub struct TuiRuntimeDeps {
    startup: TuiStartupDeps,
    services: TuiServiceDeps,
    channels: TuiChannelDeps,
}

/// Test-visible bundle of optional actors.
pub struct SpawnedOptionalActors {
    pub executor_join: Option<TaskJoin>,
    pub supervisor_handle: Option<actors::SupervisorHandle>,
    pub chat_join: Option<TaskJoin>,
    pub chat_provider: Arc<dyn ChatProvider>,
    pub consumer_handles: ConsumerHandles,
}

pub struct TuiStartupDeps {
    config: AppConfig,
    renderer: AppRenderer,
    orchestrator: actors::DeterministicOrchestratorHandle,
}

pub struct TuiServiceDeps {
    chat_provider: Arc<dyn ChatProvider>,
    session: actors::SessionHandle,
    ask: actors::AskHandle,
    file_scanner: actors::FileScannerHandle,
    guided_plan: actors::GuidedPlanHandle,
}

pub struct TuiChannelDeps {
    output_rx: AgentOutputReceiver,
    supervisor: Option<SupervisorReceiver>,
    feed_tx: mpsc::Sender<FeedEntry>,
    feed_rx: mpsc::Receiver<FeedEntry>,
}

struct RuntimeActors {
    domain: SpawnedDomainActors,
    planning: SpawnedPlanningActors,
    chat: ChatRuntime,
    supervisor: SupervisorRuntime,
    consumer_handles: ConsumerHandles,
}

struct RuntimeUiChannels {
    feed_tx: mpsc::Sender<FeedEntry>,
    feed_rx: mpsc::Receiver<FeedEntry>,
    sub_actors: TuiSubActorHandles,
}

struct SpawnAppFinalizeArgs<'a> {
    core: CoreRuntime,
    config: &'a AppConfig,
    renderer: AppRenderer,
    actors: RuntimeActors,
    ui_channels: RuntimeUiChannels,
}

struct ChatParts {
    provider: Arc<dyn ChatProvider>,
    output_rx: AgentOutputReceiver,
    join: Option<TaskJoin>,
}

struct SupervisorParts {
    rx: Option<SupervisorReceiver>,
    join: Option<TaskJoin>,
    handle: Option<actors::SupervisorHandle>,
}

struct UnpackedRuntimeActors {
    domain: SpawnedDomainActors,
    planning: SpawnedPlanningActors,
    chat: ChatParts,
    supervisor: SupervisorParts,
    consumer_handles: ConsumerHandles,
}

struct NonUiAppActors {
    domain: SpawnedDomainActors,
    supervisor: SupervisorRuntime,
    chat: ChatRuntime,
    planning: SpawnedPlanningActors,
}

pub struct TuiBuildCore<'a> {
    pub(crate) config: &'a AppConfig,
    pub(crate) renderer: AppRenderer,
    pub(crate) domain: &'a SpawnedDomainActors,
    pub(crate) planning: &'a SpawnedPlanningActors,
    pub(crate) chat_provider: Arc<dyn ChatProvider>,
}

pub struct TuiBuildChannels {
    pub(crate) output_rx: AgentOutputReceiver,
    pub(crate) supervisor_rx: Option<SupervisorReceiver>,
    pub(crate) feed_tx: mpsc::Sender<FeedEntry>,
    pub(crate) feed_rx: mpsc::Receiver<FeedEntry>,
}

pub struct SpawnedTuiDeps {
    startup: TuiStartupDeps,
    services: TuiServiceDeps,
    channels: TuiChannelDeps,
}

// ── Sub-modules ───────────────────────────────────────────────────────────────

mod app_runtime;
mod chat_provider;
mod domain;
mod infrastructure;
mod lifecycle;
mod supervisor;
pub mod task_runner;
mod tui_wiring;

// ── Public re-imports ──────────────────────────────────────────────────────

pub use infrastructure::BuildRegistryArgs;
pub use infrastructure::OptionalToolArgs;
pub use infrastructure::RegistryDirectoryScope;
pub use infrastructure::build_registry;
pub use infrastructure::spawn_core_runtime;
pub use infrastructure::take_openrouter_feed_rx;
pub use lifecycle::{await_runtime, shutdown_runtime};

// Test and internal wiring re-imports
pub use app_runtime::{
    AppRuntimeConfigRef, actor_runtime, build_run_runtime, forward_reply_to_broadcast,
    spawn_app_runtime, spawn_deterministic_orchestrator_runtime,
    spawn_root_deterministic_orchestrator_runtime,
};
pub use chat_provider::{EndpointRoutingChatProvider, spawn_chat_runtime};
pub use domain::{
    DomainRuntimeConfigRef, spawn_agent_runtime, spawn_ask_runtime, spawn_domain_actors,
    spawn_planning_actors,
};
pub use supervisor::{spawn_supervisor_runtime, wire_supervisor};
pub use tui_wiring::{
    build_spawned_tui_deps, build_tui_deps, build_tui_runtime_deps, spawn_consumer_actors,
    spawn_tui_actor, spawn_tui_runtime, spawn_tui_sub_actors, take_query_rx,
};

/// Build a `DomainRuntimeConfigRef` from app/runtime config references.
pub fn domain_runtime_config_ref<'a>(
    config: &'a AppConfig,
    program_settings: &'a ProgramSettings,
) -> DomainRuntimeConfigRef<'a> {
    DomainRuntimeConfigRef {
        config,
        program_settings,
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Forward `StreamChunk` items from an LLM reply channel to the agent output broadcast.
///
/// Reads chunks from `rx` until the channel closes. Converts each chunk to the
/// matching `AgentOutput` variant and sends it on `output_tx` so automated LLM
/// responses flow through the same rendering path as regular agent responses.
///
/// Called by the auto-message bridge in `spawn_app_runtime` for each automated
/// message so the LLM response is not silently discarded.
/// Spawn all actors, run until the user quits, then shut down cleanly.
///
/// Entry point for actor wiring and orchestration.
///
/// Spawns actors in dependency order - each actor receives only the handles
/// of actors it depends on, never raw shared state:
///
/// 1. `LlmActor` (owns config)
/// 2. `ToolActor` (owns tool registry)
/// 3. `CacheActor` (watches src/ for file changes)
/// 4. `AgentActor` (owns LLM + tool + cache handles + agent config + services)
/// 5. `SessionActor` (owns default endpoint selection)
/// 6. `TuiActor` (owns terminal, agent handle, session handle, output feed)
///
/// Blocks until the TUI signals shutdown, then shuts down actors in reverse
/// dependency order and awaits all join handles before returning.
///
/// This function is instrumented with `tracing::instrument` for observability,
/// emitting debug-level events for all major phases of initialization, runtime,
/// and shutdown to support tracing and performance analysis.
#[tracing::instrument(skip_all, level = "info")]
pub async fn run(
    run_config: RunConfig,
    renderer: AppRenderer,
    session_secs: TimestampSecs,
) -> anyhow::Result<()> {
    let mut runtime = wire_runtime(&run_config, renderer, session_secs).await;
    runtime.app.handles.primary.ui.tui.wait_for_shutdown().await;

    // Save user settings before shutting down the agent
    save_user_settings_on_exit(&runtime).await;

    shutdown_runtime(&runtime);
    await_runtime(runtime).await;
    Ok(())
}

/// Query the current agent state and save user settings on exit.
///
/// This is called before shutdown to ensure the agent is still responsive.
/// Captures the last endpoint and selected model, then persists them to disk
/// so they can be restored on the next session.
async fn save_user_settings_on_exit(runtime: &RunRuntime) {
    use augur_core::config::user_settings;
    use augur_domain::domain::thinking_mode::ReasoningEffort;

    let agent = &runtime.app.handles.primary.domain.agent;
    let agent_state = agent.get_state().await;
    let current_settings = user_settings::load_user_settings();
    let effort = current_settings
        .last_reasoning_effort
        .as_deref()
        .and_then(ReasoningEffort::parse_optional);

    // If we have a last endpoint, save it along with the model
    if let Some(endpoint) = &agent_state.last_endpoint {
        user_settings::save_user_settings(
            Some(endpoint),
            agent_state.selected_model.as_ref(),
            effort.as_ref(),
        );
    }
}

async fn wire_runtime(
    run_config: &RunConfig,
    renderer: AppRenderer,
    session_secs: TimestampSecs,
) -> RunRuntime {
    let core = spawn_infrastructure(
        &run_config.config,
        &run_config.program_settings,
        session_secs,
    );
    spawn_app_runtime(
        AppRuntimeConfigRef {
            config: &run_config.config,
            program_settings: &run_config.program_settings,
        },
        renderer,
        core,
    )
    .await
}

/// Spawn and wire infrastructure actors.
/// These actors form the foundation that all higher-level actors depend on:
/// - `LlmActor` - owns LLM config and handles model requests
/// - `FileReadActor` - reads files from allowed directories
/// - `CacheActor` - watches source directory for file changes (optional)
/// - `ToolActor` - orchestrates all tools used by agents
/// - `LoggerActor` - persists log entries to disk
pub fn spawn_infrastructure(
    config: &AppConfig,
    program_settings: &ProgramSettings,
    session_secs: TimestampSecs,
) -> CoreRuntime {
    spawn_core_runtime(config, program_settings, session_secs)
}

/// Spawn the deterministic orchestrator through the wiring composition surface.
///
/// Inputs:
/// - `repo_root`: repository root used to resolve deterministic workflow files.
///
/// Returns:
/// - A live [`actors::DeterministicOrchestratorHandle`] connected to a spawned actor task.
///
/// Invariants:
/// - Workflow loading remains rooted at `repo_root`.
///
/// Side effects:
/// - Spawns the deterministic orchestrator Tokio task immediately.
pub fn spawn_deterministic_orchestrator(
    repo_root: impl Into<std::path::PathBuf>,
) -> actors::DeterministicOrchestratorHandle {
    actors::deterministic_orchestrator::deterministic_orchestrator_actor::spawn(repo_root)
}

// Sends shutdown signals to all runtime actors in layer-aware order.
//
// See lifecycle::shutdown_runtime for full documentation.
// Block until all spawned actor tasks have completed.
//
// See lifecycle::await_runtime for full documentation.
