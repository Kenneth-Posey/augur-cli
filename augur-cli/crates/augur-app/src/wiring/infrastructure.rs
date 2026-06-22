use super::{
    CoreActorJoins, CoreControl, CoreHandles, CoreIoHandles, CoreRuntime, CoreRuntimeContext,
    CoreServiceHandles, CoreStartup, CoreSupportJoins, QueryChannels, TaskJoin,
    DEFAULT_CACHE_WATCH_DIR,
};
use augur_core::actors;
use augur_core::actors::cache::handle::CacheHandle;
use augur_core::actors::catalog_manager::catalog_manager_actor as catalog_manager;
use augur_core::actors::command::command_actor::build as build_command;
use augur_core::actors::file_read::FileReadHandle;
use augur_core::actors::history_adapter::handle::HistoryAdapterHandle;
use augur_core::actors::history_adapter::history_adapter_actor::{
    spawn as spawn_history_adapter, HistoryAdapterConfig,
};
use augur_core::actors::lsp::lsp_actor::{spawn as spawn_lsp_actor, LspActorConfig};
use augur_core::actors::lsp::LspHandle;
use augur_core::actors::tool::InlineToolExecutor;
use augur_core::persistence::{handle::PersistenceHandle, store};
use augur_core::tools::builtin::{
    file_append::FileAppendTool, file_create::FileCreateTool, file_insert::FileInsertTool,
    file_line_count::FileLineCountTool, file_read::FileReadTool,
    file_read_range::FileReadRangeTool, file_replace::FileReplaceTool, file_slice::FileSliceTool,
    list_directory::ListDirectoryTool, lsp_query::LspQueryTool, query_user::QueryUserTool,
    refresh_cache_file::RefreshCacheFileTool, scoped_shell_exec::ScopedShellExecTool,
    set_working_file::SetWorkingFileTool, shell_exec::ShellExecTool, size_check::SizeCheckTool,
};
use augur_core::tools::registry::ToolRegistry;
use augur_domain::config::install_path::{effective_repo_root, resolve_install_path};
use augur_domain::config::provider_catalog::{default_provider_catalog_dir, load_provider_catalog};
use augur_domain::config::types::{AppConfig, ProgramSettings};
use augur_domain::domain::channels::{
    AGENT_FEED_CAPACITY, AGENT_OUTPUT_CAPACITY, HISTORY_FEED_CAPACITY, QUERY_USER_CHANNEL_CAPACITY,
    SPAWN_AGENT_CHANNEL_CAPACITY,
};
use augur_domain::domain::feeds::HistoryFeedMessage;
use augur_domain::domain::newtypes::TimestampSecs;
use augur_domain::domain::task_types::{
    AgentSpecName, InstructionPrefix, RepoRoot, SpawnAgentRequest,
};
use augur_domain::domain::types::Message;
use augur_domain::domain::StringNewtype;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use augur_provider_openrouter::actors::openrouter_orchestrator::openrouter_orchestrator_actor::{
    OpenRouterOrchestratorArgs, OrchestratorIoChannels, OrchestratorRuntimeHandles,
    OrchestratorTaskConfig,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Optional tool handles registered conditionally in [`crate::wiring::build_registry`].
///
/// Bundles the two optional tool configurations so [`BuildRegistryArgs`] stays
/// within the 5-field limit. Construct with struct literal syntax (2 fields;
/// `bon::Builder` not needed).
pub struct OptionalToolArgs {
    /// When `Some`, registers `SpawnAgentTool` for OpenRouter background agent support.
    pub spawn_agent: Option<SpawnAgentConfig>,
    /// When `Some`, registers `LspQueryTool` for LSP operations.
    pub lsp: Option<LspHandle>,
}

/// Arguments for [`crate::wiring::build_registry`].
///
/// Bundles the five inputs required to construct the built-in tool registry so
/// the function signature stays within the three-parameter limit.
#[derive(bon::Builder)]
pub struct RegistryDirectoryScope {
    /// Directories that `file_create` is permitted to write to.
    pub allowed_dirs: Vec<std::path::PathBuf>,
    /// Directories that are forbidden.
    pub excluded_dirs: Vec<std::path::PathBuf>,
}

#[derive(bon::Builder)]
/// Arguments required to build the runtime `ToolRegistry`.
pub struct BuildRegistryArgs {
    /// Sending half of the channel the TUI actor listens on for query requests
    /// from the `query_user` tool.
    pub query_tx: mpsc::Sender<QueryUserRequest>,
    /// Handle to the running `FileReadActor` shared by the two range tools.
    pub file_read: FileReadHandle,
    /// When `Some`, also registers `set_working_file` and `refresh_cache_file`.
    pub cache: Option<CacheHandle>,
    /// Allowed/excluded directory constraints for filesystem tools.
    pub dirs: RegistryDirectoryScope,
    /// Optional tool handles for conditionally registered tools.
    pub optional: OptionalToolArgs,
}

/// Configuration for the optional `SpawnAgentTool` registration.
///
/// Bundles the channel sender and the list of available agent names so the tool
/// description can enumerate valid names for the model.
pub struct SpawnAgentConfig {
    /// Sending half of the spawn-agent request channel.
    pub tx: mpsc::Sender<SpawnAgentRequest>,
    /// Names of `.github/agents/<name>.agent.md` files found at startup.
    pub available_agents: Vec<AgentSpecName>,
    /// OpenRouter orchestrator handle used by await/status task tools.
    pub orchestrator:
        augur_provider_openrouter::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle,
}

#[derive(bon::Builder)]
struct OpenRouterToolExecutorArgs {
    query_tx: mpsc::Sender<QueryUserRequest>,
    file_read: FileReadHandle,
    allowed_dirs: Vec<std::path::PathBuf>,
    excluded_dirs: Vec<std::path::PathBuf>,
    lsp: Option<LspHandle>,
    repo_root: RepoRoot,
}

#[derive(bon::Builder)]
struct OpenRouterRuntimeInput {
    config: AppConfig,
    llm: augur_provider_openrouter::actors::LlmHandle,
    tool_executor_args: OpenRouterToolExecutorArgs,
}

#[derive(bon::Builder)]
struct OpenRouterRuntimeWiring {
    spawn_agent_tx: mpsc::Sender<SpawnAgentRequest>,
    available_agents: Vec<AgentSpecName>,
    orchestrator_handle:
        augur_provider_openrouter::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle,
    active_model_handle: actors::ActiveModelHandle,
    openrouter_feed_rx: mpsc::Receiver<augur_domain::domain::types::FeedEntry>,
}

struct CoreSpawnChannels {
    agent_tx: tokio::sync::broadcast::Sender<augur_domain::domain::types::AgentOutput>,
    query_tx: mpsc::Sender<QueryUserRequest>,
    query_rx: mpsc::Receiver<QueryUserRequest>,
}

struct CoreSpawnServices {
    llm: CoreServiceTask<augur_provider_openrouter::actors::LlmHandle>,
    dirs: Vec<std::path::PathBuf>,
    excluded_dirs: Vec<std::path::PathBuf>,
    file_read: CoreServiceTask<FileReadHandle>,
}

struct CoreServiceTask<T> {
    join: TaskJoin,
    handle: T,
}

struct CoreSpawnSupport {
    cache_handle: Option<CacheHandle>,
    lsp_join: TaskJoin,
    lsp_handle: LspHandle,
    openrouter: OpenRouterRuntimeWiring,
}

struct CoreSpawnObservability {
    logger_join: TaskJoin,
    logger_handle: actors::LoggerHandle,
    history_adapter_handle: HistoryAdapterHandle,
    catalog_manager_handle: actors::catalog_manager::CatalogManagerHandle,
}

struct CoreSpawnWiring {
    channels: CoreSpawnChannels,
    services: CoreSpawnServices,
    support: CoreSpawnSupport,
    observability: CoreSpawnObservability,
}

/// Arguments for `spawn_core_wiring`, bundling session scalars and the logger
/// so the function signature stays within the 3-parameter limit.
struct CoreSpawnWiringArgs<'a> {
    config: &'a AppConfig,
    program_settings: &'a ProgramSettings,
    session_id: &'a str,
    session_secs: TimestampSecs,
}

/// Arguments for [`build_core_runtime`], bundling session-level scalars
/// so the function signature stays within the 3-parameter limit.
struct BuildCoreRuntimeArgs<'a> {
    config: &'a AppConfig,
    session_id: &'a str,
}

/// Build a [`ToolRegistry`] pre-loaded with all built-in tools.
///
/// Always registers `shell_exec`, `file_read`, `file_create`, `file_append`,
/// `query_user`, `file_read_range`, `file_line_count`, `size_check`, and
/// `list_directory`. When `cache` is `Some`, also registers `set_working_file`
/// and `refresh_cache_file`. When `optional.spawn_agent` is `Some`, also
/// registers `SpawnAgentTool` so the main agent session exposes the `task`
/// tool to the model with the list of available agent names embedded in the
/// description. When `optional.lsp` is `Some`, also registers `LspQueryTool`
/// for LSP operations. `query_tx` is the sending half of the channel the TUI
/// actor listens on for query requests from the `query_user` tool. `file_read`
/// is the handle to the running `FileReadActor` shared by the two range tools.
/// Called once at startup before spawning the `ToolActor`. Extend this
/// function when adding new built-in tools.
pub fn build_registry(args: BuildRegistryArgs) -> ToolRegistry {
    let BuildRegistryArgs {
        query_tx,
        file_read,
        cache,
        dirs,
        optional,
    } = args;
    let RegistryDirectoryScope {
        allowed_dirs,
        excluded_dirs,
    } = dirs;
    let mut registry = ToolRegistry::new();
    registry.register(ShellExecTool);
    registry.register(FileReadTool::new(file_read.clone()));
    registry.register(FileCreateTool::new(allowed_dirs.clone()));
    registry.register(FileAppendTool::new(allowed_dirs.clone()));
    registry.register(FileInsertTool::new(allowed_dirs.clone()));
    registry.register(FileReplaceTool::new(allowed_dirs.clone()));
    registry.register(FileSliceTool::new(allowed_dirs.clone()));
    registry.register(QueryUserTool::new(query_tx));
    registry.register(FileReadRangeTool::new(file_read.clone()));
    registry.register(FileLineCountTool::new(file_read));
    registry.register(SizeCheckTool::new(
        allowed_dirs.clone(),
        excluded_dirs.clone(),
    ));
    registry.register(ListDirectoryTool::new(allowed_dirs, excluded_dirs));
    if let Some(ch) = cache {
        registry.register(SetWorkingFileTool::new(ch.clone()));
        registry.register(RefreshCacheFileTool::new(ch));
    }
    if let Some(cfg) = optional.spawn_agent {
        use augur_core::tools::builtin::spawn_agent::SpawnAgentTool;
        use augur_core::tools::builtin::task_await::TaskAwaitTool;
        use augur_core::tools::builtin::task_status::TaskStatusTool;
        use augur_domain::domain::task_types::{SpawnAgentHandle, TaskDepth};
        let orchestrator = Arc::new(cfg.orchestrator);
        registry.register(
            SpawnAgentTool::builder()
                .handle(SpawnAgentHandle(cfg.tx))
                .depth(TaskDepth::root())
                .available_agents(cfg.available_agents)
                .build(),
        );
        registry.register(
            TaskAwaitTool::builder()
                .orchestrator(orchestrator.clone())
                .build(),
        );
        registry.register(TaskStatusTool::builder().orchestrator(orchestrator).build());
    }
    if let Some(lsp_handle) = optional.lsp {
        registry.register(LspQueryTool::new(lsp_handle));
    }
    registry
}

/// Spawn all core infrastructure actors and return a [`CoreRuntime`].
///
/// Creates the shared agent output broadcast channel, then spawns the LLM,
/// file-read, tool, logger, token-tracker, history-adapter, and
/// catalog-manager actors. Builds the built-in tool registry and the CLI
/// command descriptor. `session_secs` is forwarded to the logger to name the
/// session log file. The query-user channel receiver is stored in
/// `CoreRuntime::query.rx` and must be consumed exactly once by the TUI actor.
/// A spawn-agent channel is created and the sender is registered as
/// `SpawnAgentTool` in the tool registry; the receiver is consumed by an
/// OpenRouter-orchestrator bridge task started here in infrastructure wiring.
pub fn spawn_core_runtime(
    config: &AppConfig,
    program_settings: &ProgramSettings,
    session_secs: TimestampSecs,
) -> CoreRuntime {
    let session_id = uuid::Uuid::new_v4().to_string();
    let args = BuildCoreRuntimeArgs {
        config,
        session_id: &session_id,
    };
    build_core_runtime(
        args,
        spawn_core_wiring(CoreSpawnWiringArgs {
            config,
            program_settings,
            session_id: &session_id,
            session_secs,
        }),
    )
}

/// Take the OpenRouter feed receiver from `core`.
///
/// This receiver carries `FeedEntry` task lifecycle/status events that the TUI
/// uses for agent-panel updates (including spinner semantics via task start/end).
/// It must be taken at most once. A second call returns a closed receiver.
pub fn take_openrouter_feed_rx(
    core: &mut CoreRuntime,
) -> mpsc::Receiver<augur_domain::domain::types::FeedEntry> {
    match core.context.control.openrouter_feed_rx.take() {
        Some(rx) => rx,
        None => {
            tracing::error!(
                "take_openrouter_feed_rx: receiver already consumed - returning closed channel"
            );
            let (_tx, rx) = mpsc::channel(1);
            rx
        }
    }
}

fn spawn_core_wiring(args: CoreSpawnWiringArgs<'_>) -> CoreSpawnWiring {
    let CoreSpawnWiringArgs {
        config,
        program_settings,
        session_id,
        session_secs,
    } = args;
    let (agent_tx, _) = tokio::sync::broadcast::channel(*AGENT_OUTPUT_CAPACITY);
    let observability = {
        let (logger_join, logger_handle) =
            actors::logger::logger_actor::spawn_with_session(log_dir(config), session_secs);
        let history_adapter_handle = spawn_history_logging_pipeline(&logger_handle);
        let catalog_manager_handle = catalog_manager::spawn();
        CoreSpawnObservability {
            logger_join,
            logger_handle,
            history_adapter_handle,
            catalog_manager_handle,
        }
    };
    let (llm_join, llm_handle) = augur_provider_openrouter::actors::llm::llm_actor::spawn(
        config.clone(),
        agent_tx.clone(),
        session_id.to_string(),
        observability.logger_handle.clone(),
    );
    let (query_tx, query_rx) = mpsc::channel::<QueryUserRequest>(*QUERY_USER_CHANNEL_CAPACITY);
    let dirs = allowed_dirs(config);
    let excluded_dirs = program_settings.excluded_directory_paths();
    let (file_read_join, file_read_handle) =
        actors::file_read::file_read_actor::spawn(dirs.clone());
    let cache_handle = spawn_cache_handle();
    let (lsp_join, lsp_handle) = spawn_lsp_actor(lsp_config());
    let repo_root = RepoRoot::new(
        effective_repo_root()
            .to_string_lossy()
            .to_string(),
    );
    let openrouter = spawn_openrouter_runtime(
        OpenRouterRuntimeInput::builder()
            .config(config.clone())
            .llm(llm_handle.clone())
            .tool_executor_args(
                OpenRouterToolExecutorArgs::builder()
                    .query_tx(query_tx.clone())
                    .file_read(file_read_handle.clone())
                    .allowed_dirs(allowed_dirs(config))
                    .excluded_dirs(excluded_dirs.clone())
                    .maybe_lsp(Some(lsp_handle.clone()))
                    .repo_root(repo_root)
                    .build(),
            )
            .build(),
    );
    CoreSpawnWiring {
        channels: CoreSpawnChannels {
            agent_tx,
            query_tx,
            query_rx,
        },
        services: CoreSpawnServices {
            llm: CoreServiceTask {
                join: llm_join,
                handle: llm_handle,
            },
            dirs,
            excluded_dirs,
            file_read: CoreServiceTask {
                join: file_read_join,
                handle: file_read_handle,
            },
        },
        support: CoreSpawnSupport {
            cache_handle,
            lsp_join,
            lsp_handle,
            openrouter,
        },
        observability,
    }
}

fn build_core_runtime(args: BuildCoreRuntimeArgs<'_>, wiring: CoreSpawnWiring) -> CoreRuntime {
    let BuildCoreRuntimeArgs {
        config,
        session_id,
    } = args;
    let CoreSpawnWiring {
        channels,
        services,
        support,
        observability,
    } = wiring;
    let CoreSpawnChannels {
        agent_tx,
        query_tx,
        query_rx,
    } = channels;
    let CoreSpawnServices {
        llm,
        dirs,
        excluded_dirs,
        file_read,
    } = services;
    let CoreServiceTask {
        join: llm_join,
        handle: llm_handle,
    } = llm;
    let CoreServiceTask {
        join: file_read_join,
        handle: file_read_handle,
    } = file_read;
    let CoreSpawnSupport {
        cache_handle,
        lsp_join,
        lsp_handle,
        openrouter,
    } = support;
    let OpenRouterRuntimeWiring {
        spawn_agent_tx,
        available_agents,
        orchestrator_handle,
        active_model_handle,
        openrouter_feed_rx,
    } = openrouter;
    let registry = build_registry(
        BuildRegistryArgs::builder()
            .query_tx(query_tx.clone())
            .file_read(file_read_handle.clone())
            .maybe_cache(cache_handle.clone())
            .dirs(
                RegistryDirectoryScope::builder()
                    .allowed_dirs(dirs)
                    .excluded_dirs(excluded_dirs)
                    .build(),
            )
            .optional(OptionalToolArgs {
                spawn_agent: Some(SpawnAgentConfig {
                    tx: spawn_agent_tx,
                    available_agents,
                    orchestrator: orchestrator_handle.clone(),
                }),
                lsp: Some(lsp_handle.clone()),
            })
            .build(),
    );
    let shutdown_lsp_handle = lsp_handle;
    let command = build_command(registry.definitions());
    let (tool_join, tool_handle) = actors::tool::tool_actor::spawn(registry);
    let (startup, token_tracker_join) = load_startup_state(config, session_id);
    let CoreSpawnObservability {
        logger_join,
        logger_handle,
        history_adapter_handle,
        catalog_manager_handle,
    } = observability;
    let actor_joins = CoreActorJoins::builder()
        .llm(llm_join)
        .file_read(file_read_join)
        .tool(tool_join)
        .build();
    let support_joins = CoreSupportJoins::builder()
        .logger(logger_join)
        .token_tracker(token_tracker_join)
        .lsp(lsp_join)
        .build();
    let services = CoreServiceHandles {
        llm: llm_handle,
        file_read: file_read_handle,
        tool: tool_handle,
    };
    let io = CoreIoHandles {
        logger: logger_handle,
        history_adapter: history_adapter_handle,
    };
    let handles = CoreHandles {
        services,
        cache: cache_handle,
        catalog_manager: catalog_manager_handle,
        io,
    };
    let query = QueryChannels {
        _tx: query_tx,
        rx: Some(query_rx),
    };
    let control = CoreControl {
        command,
        agent_tx,
        openrouter_orchestrator_handle: orchestrator_handle,
        openrouter_active_model_handle: active_model_handle,
        openrouter_feed_rx: Some(openrouter_feed_rx),
        lsp_handle: shutdown_lsp_handle,
    };
    CoreRuntime {
        actor_joins,
        support_joins,
        handles,
        context: CoreRuntimeContext {
            query,
            startup,
            control,
        },
    }
}

fn spawn_openrouter_runtime(input: OpenRouterRuntimeInput) -> OpenRouterRuntimeWiring {
    let OpenRouterRuntimeInput {
        config,
        llm,
        tool_executor_args,
    } = input;
    let (spawn_agent_tx, spawn_agent_rx) = openrouter_spawn_channel();
    let available_agents = scan_available_agents();
    let openrouter_active_model_handle = actors::active_model::spawn();
    let (openrouter_feed_tx, openrouter_feed_rx) = openrouter_feed_channel();
    let OpenRouterToolExecutorArgs {
        query_tx,
        file_read,
        allowed_dirs,
        excluded_dirs,
        lsp,
        repo_root,
    } = tool_executor_args;
    let tool_executor = build_openrouter_tool_executor(
        OpenRouterToolExecutorArgs::builder()
            .query_tx(query_tx)
            .file_read(file_read)
            .allowed_dirs(allowed_dirs)
            .excluded_dirs(excluded_dirs)
            .maybe_lsp(lsp)
            .repo_root(repo_root.clone())
            .build(),
    );
    let openrouter_orchestrator_handle = spawn_openrouter_orchestrator(
        SpawnOpenRouterOrchestratorArgs::builder()
            .config(&config)
            .llm(llm)
            .active_model(openrouter_active_model_handle.clone())
            .tool_executor(tool_executor)
            .feed_tx(openrouter_feed_tx)
            .repo_root(repo_root)
            .build(),
    );
    spawn_openrouter_spawn_agent_bridge(spawn_agent_rx, openrouter_orchestrator_handle.clone());
    OpenRouterRuntimeWiring::builder()
        .spawn_agent_tx(spawn_agent_tx)
        .available_agents(available_agents)
        .orchestrator_handle(openrouter_orchestrator_handle)
        .active_model_handle(openrouter_active_model_handle)
        .openrouter_feed_rx(openrouter_feed_rx)
        .build()
}

fn openrouter_spawn_channel() -> (
    mpsc::Sender<SpawnAgentRequest>,
    mpsc::Receiver<SpawnAgentRequest>,
) {
    mpsc::channel::<SpawnAgentRequest>(*SPAWN_AGENT_CHANNEL_CAPACITY)
}

fn openrouter_feed_channel() -> (
    mpsc::Sender<augur_domain::domain::types::FeedEntry>,
    mpsc::Receiver<augur_domain::domain::types::FeedEntry>,
) {
    mpsc::channel::<augur_domain::domain::types::FeedEntry>(*AGENT_FEED_CAPACITY)
}

#[derive(bon::Builder)]
struct SpawnOpenRouterOrchestratorArgs<'a> {
    config: &'a AppConfig,
    llm: augur_provider_openrouter::actors::LlmHandle,
    active_model: actors::ActiveModelHandle,
    tool_executor: InlineToolExecutor,
    feed_tx: mpsc::Sender<augur_domain::domain::types::FeedEntry>,
    repo_root: RepoRoot,
}

fn spawn_openrouter_orchestrator(
    args: SpawnOpenRouterOrchestratorArgs<'_>,
) -> augur_provider_openrouter::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle
{
    let SpawnOpenRouterOrchestratorArgs {
        config,
        llm,
        active_model,
        tool_executor,
        feed_tx,
        repo_root,
    } = args;
    let instruction_prefix = load_openrouter_background_instruction_prefix(repo_root.as_ref());
    let (_join, handle) =
        augur_provider_openrouter::actors::openrouter_orchestrator::openrouter_orchestrator_actor::spawn(
            OpenRouterOrchestratorArgs::builder()
                .runtime(
                    OrchestratorRuntimeHandles::builder()
                        .llm(llm)
                        .active_model(active_model)
                        .tool_executor(tool_executor)
                        .build(),
                )
                .io(OrchestratorIoChannels { feed_tx })
                .config(
                    OrchestratorTaskConfig::builder()
                        .allowed_dirs(allowed_dirs(config))
                        .instruction_prefix(std::sync::Arc::new(instruction_prefix))
                        .repo_root(repo_root)
                        .max_parallel_workers(4)
                        .build(),
                )
                .build(),
        );
    handle
}

fn load_openrouter_background_instruction_prefix(repo_root: &str) -> InstructionPrefix {
    let files = match load_background_instruction_file_list() {
        Some(files) => files,
        None => return InstructionPrefix(vec![]),
    };
    if files.is_empty() {
        return InstructionPrefix(vec![]);
    }
    InstructionPrefix(load_background_instruction_messages(&files, repo_root))
}

fn load_background_instruction_file_list() -> Option<Vec<String>> {
    let catalog_dir = default_provider_catalog_dir();
    let catalog = load_provider_catalog(
        &catalog_dir,
        augur_domain::config::types::Provider::OpenRouter,
    )
    .ok()
    .flatten()?;
    let openrouter = catalog.openrouter?;
    if openrouter.background_instruction_files.is_empty() {
        Some(openrouter.instruction_files)
    } else {
        Some(openrouter.background_instruction_files)
    }
}

fn load_background_instruction_messages(files: &[String], repo_root: &str) -> Vec<Message> {
    let mut messages = Vec::with_capacity(files.len());
    for path in files {
        if let Some(message) = load_background_instruction_message(repo_root, path) {
            messages.push(message);
        }
    }
    messages
}

fn load_background_instruction_message(repo_root: &str, path: &str) -> Option<Message> {
    // Try CWD-relative first
    let cwd_abs = format!("{repo_root}/{path}");
    if std::path::Path::new(&cwd_abs).exists() {
        match std::fs::read_to_string(&cwd_abs) {
            Ok(content) => return Some(Message::user(format!("[FILE: {path}]\n{content}"))),
            Err(error) => {
                tracing::warn!(
                    path = %path,
                    error = %error,
                    "background instruction file not readable; skipping"
                );
                return None;
            }
        }
    }
    // Fall back to installed config path
    let installed = resolve_install_path(path);
    match std::fs::read_to_string(&installed) {
        Ok(content) => Some(Message::user(format!("[FILE: {path}]\n{content}"))),
        Err(error) => {
            tracing::warn!(
                path = %path,
                error = %error,
                "background instruction file not readable; skipping"
            );
            None
        }
    }
}

fn build_openrouter_tool_executor(args: OpenRouterToolExecutorArgs) -> InlineToolExecutor {
    let OpenRouterToolExecutorArgs {
        query_tx,
        file_read,
        allowed_dirs,
        excluded_dirs,
        lsp,
        repo_root,
    } = args;
    let mut registry = ToolRegistry::new();
    registry.register(ScopedShellExecTool::new(repo_root));
    registry.register(FileReadTool::new(file_read.clone()));
    registry.register(FileCreateTool::new(allowed_dirs.clone()));
    registry.register(FileAppendTool::new(allowed_dirs.clone()));
    registry.register(FileInsertTool::new(allowed_dirs.clone()));
    registry.register(FileReplaceTool::new(allowed_dirs.clone()));
    registry.register(FileSliceTool::new(allowed_dirs.clone()));
    registry.register(QueryUserTool::new(query_tx));
    registry.register(FileReadRangeTool::new(file_read.clone()));
    registry.register(FileLineCountTool::new(file_read));
    registry.register(SizeCheckTool::new(
        allowed_dirs.clone(),
        excluded_dirs.clone(),
    ));
    registry.register(ListDirectoryTool::new(allowed_dirs, excluded_dirs));
    if let Some(lsp_handle) = lsp {
        registry.register(LspQueryTool::new(lsp_handle));
    }
    InlineToolExecutor::new(registry)
}

fn spawn_openrouter_spawn_agent_bridge(
    mut spawn_agent_rx: mpsc::Receiver<SpawnAgentRequest>,
    orchestrator: augur_provider_openrouter::actors::openrouter_orchestrator::handle::OpenRouterOrchestratorHandle,
) {
    tokio::spawn(async move {
        while let Some(request) = spawn_agent_rx.recv().await {
            if let Err(error) = orchestrator.enqueue_request(request, None) {
                tracing::warn!(
                    "failed to enqueue spawn-agent request into OpenRouter orchestrator: {error}"
                );
            }
        }
    });
}

fn spawn_history_logging_pipeline(logger_handle: &actors::LoggerHandle) -> HistoryAdapterHandle {
    let (history_tx, history_rx) = mpsc::channel::<HistoryFeedMessage>(*HISTORY_FEED_CAPACITY);
    let (_, history_adapter_handle) = spawn_history_adapter(HistoryAdapterConfig {
        history_tx,
        capacity: *HISTORY_FEED_CAPACITY,
    });
    let logger_clone = logger_handle.clone();
    tokio::spawn(async move {
        let mut history_rx = history_rx;
        while let Some(entry) = history_rx.recv().await {
            logger_clone.log_history_entry(entry);
        }
    });
    history_adapter_handle
}

fn allowed_dirs(config: &AppConfig) -> Vec<std::path::PathBuf> {
    config
        .agent
        .allowed_dirs
        .iter()
        .map(|path| std::path::PathBuf::from(path.as_str()))
        .collect()
}

fn spawn_cache_handle() -> Option<CacheHandle> {
    actors::cache::cache_actor::spawn(std::path::PathBuf::from(DEFAULT_CACHE_WATCH_DIR))
        .map_err(
            |e| tracing::warn!(error = %e, "cache actor failed to start; file caching disabled"),
        )
        .ok()
}

fn load_startup_state(config: &AppConfig, session_id: &str) -> (CoreStartup, TaskJoin) {
    let base_sessions_dir = store::resolve_sessions_dir(config.persistence.sessions_dir.as_ref());
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let sessions_dir = store::apply_repo_subdir(base_sessions_dir, &cwd);
    let session_summaries = store::list_sessions(&sessions_dir).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to list sessions at startup");
        vec![]
    });
    let persistence = PersistenceHandle::with_session_id(
        sessions_dir.clone(),
        augur_domain::domain::SessionId::new(session_id),
    );
    let (token_tracker_join, token_tracker) = spawn_token_tracker();
    let startup = CoreStartup {
        persistence,
        sessions_dir,
        session_summaries,
        token_tracker,
    };
    (startup, token_tracker_join)
}

fn spawn_token_tracker() -> (TaskJoin, actors::TokenTrackerHandle) {
    actors::token_tracker::spawn()
}

fn log_dir(config: &AppConfig) -> std::path::PathBuf {
    let base = std::path::PathBuf::from(config.persistence.log_dir.as_str());
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    store::apply_repo_subdir(base, &cwd)
}

/// Scan `.github/agents/` in the repo directory first, then merge in any
/// additional agents from `~/.augur-cli/.github/agents/` that are not already
/// present. Returns the unique set of [`AgentSpecName`] stems.
///
/// This enables the installed config directory to add or override agent specs
/// while always picking up the full set from the active repo.
fn scan_available_agents() -> Vec<AgentSpecName> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut agents: Vec<AgentSpecName> = Vec::new();

    // Scan repo agents first (or CWD if `.github` is present; see
    // `effective_repo_root()` for the two-tier resolution).
    let repo_root = effective_repo_root();
    scan_agents_from(&mut seen, &mut agents, repo_root.join(".github/agents"));

    // Then scan the installed config directory for any additions.
    if let Ok(home) = std::env::var("HOME") {
        let install_agents = std::path::PathBuf::from(home).join(".augur-cli/.github/agents");
        if install_agents.exists() && install_agents != repo_root.join(".github/agents") {
            scan_agents_from(&mut seen, &mut agents, install_agents);
        }
    }

    agents
}

/// Helper: read `.agent.md` files from `dir`, deduplicating by stem via
/// `seen`, and push new [`AgentSpecName`]s into `out`.
fn scan_agents_from(
    seen: &mut HashSet<String>,
    out: &mut Vec<AgentSpecName>,
    dir: std::path::PathBuf,
) {
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = file_name.strip_suffix(".agent.md") else {
            continue;
        };
        if !seen.insert(stem.to_string()) {
            continue; // already added from repo
        }
        let prefixed = AgentSpecName::new(stem);
        out.push(
            augur_provider_openrouter::actors::openrouter_task::spec_loader::strip_agent_name_prefix(
                &prefixed,
            ),
        );
    }
}

/// Build an [`LspActorConfig`] rooted at the current working directory.
///
/// Derives `root_uri` from `std::env::current_dir()` as a `file://` URI.
/// Falls back to `"file:///tmp"` if the working directory is unavailable.
/// Called once at startup by `spawn_core_runtime`.
fn lsp_config() -> LspActorConfig {
    let root_uri = std::env::current_dir()
        .map(|p| format!("file://{}", p.display()))
        .unwrap_or_else(|_| "file:///tmp".to_string());
    LspActorConfig {
        root_uri: root_uri.into(),
    }
}
