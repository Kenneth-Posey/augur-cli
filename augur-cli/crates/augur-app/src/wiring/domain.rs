use super::{AskRuntime, CoreRuntime, SpawnedDomainActors, SpawnedPlanningActors, TaskJoin};
use augur_core::actors;
use augur_core::actors::agent::agent_actor::{AgentRuntime, AgentServices, AgentSpawnArgs};
use augur_domain::config::install_path::effective_repo_root;
use augur_domain::config::types::{AppConfig, ProgramSettings, Provider, find_endpoint};
use augur_domain::domain::string_newtypes::{ModelId, StringNewtype};
use augur_domain::domain::task_types::{AgentExtensions, CacheHandle as DomainCacheHandle};
use augur_domain::domain::types::FeedEntry;
use augur_domain::persistence::handle::PersistenceHandle;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
pub struct DomainRuntimeConfigRef<'a> {
    pub(crate) config: &'a AppConfig,
    pub(crate) program_settings: &'a ProgramSettings,
}

/// Spawn all domain actors and return them as a [`SpawnedDomainActors`] bundle.
///
/// Spawns the agent, session, ask-agent, and deterministic orchestrator actors
/// in dependency order. `feed_tx` is forwarded to the orchestrator so it can
/// deliver pipeline output to TUI consumers.
pub async fn spawn_domain_actors(
    runtime_config: DomainRuntimeConfigRef<'_>,
    core: &CoreRuntime,
    feed_tx: mpsc::Sender<FeedEntry>,
) -> SpawnedDomainActors {
    let agent = super::actor_runtime(spawn_agent_runtime(runtime_config.config, core).await);
    let session = super::actor_runtime(actors::session::session_actor::spawn(
        runtime_config.config.default_endpoint.clone(),
    ));
    let ask = spawn_ask_runtime(runtime_config.config, runtime_config.program_settings, core).await;

    // Build a unified dispatch runtime that routes pipeline steps to the
    // correct execution strategy (Copilot SDK vs. generic LLM tool-call loop)
    // based on the active endpoint's provider.
    let dispatch_runtime = {
        let instruction_prefix = runtime_config.config.default_endpoint.as_str();
        let prefix = if instruction_prefix.is_empty() {
            None
        } else {
            let ep = find_endpoint(
                runtime_config.config,
                &runtime_config.config.default_endpoint,
            );
            ep.map(|ep| {
                super::infrastructure::load_background_instruction_prefix_for_provider(
                    &ep.provider,
                    effective_repo_root().to_string_lossy().as_ref(),
                )
            })
        };
        let mut dr = super::dispatch_runtime::DispatchRuntime::new(
            core.handles.services.llm.clone(),
            core.handles.services.tool.clone(),
            session.handle.clone(),
            runtime_config.config.clone(),
        );
        if let Some(p) = prefix
            && !p.0.is_empty()
        {
            dr = dr.with_instruction_prefix(std::sync::Arc::new(p));
        }
        Arc::new(dr)
    };
    let deterministic_orchestrator =
        super::actor_runtime(
            actors::deterministic_orchestrator::deterministic_orchestrator_actor::spawn_with_join_and_feed_and_runtime(
                effective_repo_root(),
                feed_tx,
                dispatch_runtime,
            ),
        );

    SpawnedDomainActors {
        agent,
        session,
        ask,
        deterministic_orchestrator,
    }
}

/// Spawn the planning actors and return them as a [`SpawnedPlanningActors`] bundle.
///
/// Spawns the `FileScannerActor` (wrapped in `\`ActorRuntime\``).
pub fn spawn_planning_actors() -> SpawnedPlanningActors {
    let file_scanner = super::actor_runtime(actors::file_scanner::file_scanner_actor::spawn());
    SpawnedPlanningActors { file_scanner }
}

/// Build [`AgentSpawnArgs`] from `config` and `core` and spawn the agent actor.
///
/// Wires the LLM handle, tool handle, persistence, logger, token tracker,
/// history adapter, and agent output channel from `core` into the agent.
/// Loads the OpenRouter instruction prefix at startup (if the catalog and
/// `openrouter.instruction_files` are present) and stores it in `extensions`.
/// Injects an OpenRouter message compactor into `extensions` for manual
/// `/compact` support when using OpenRouter endpoints.
/// Resolves the default endpoint's model config to populate `auto_compact_threshold`
/// as the request-size guard threshold on `AgentRuntime`.
/// Returns the raw `(TaskJoin, AgentHandle)` pair; callers wrap it with
/// `\`super::actor_runtime\`` if an `\`ActorRuntime\`` is needed.
pub async fn spawn_agent_runtime(
    config: &AppConfig,
    core: &CoreRuntime,
) -> (TaskJoin, actors::AgentHandle) {
    // Resolve the default endpoint's model config so we can populate the
    // request-size guard threshold from the provider catalog.
    let default_endpoint_config =
        augur_domain::config::types::find_endpoint(config, &config.default_endpoint);
    let default_model_id = default_endpoint_config.map(|ep| {
        let model_name: &str = &ep.model;
        ModelId::new(model_name)
    });
    let model_config =
        augur_provider_openrouter::model_config::resolve_model_config(default_model_id.as_ref());

    let instruction_prefix = {
        let default_ep = find_endpoint(config, &config.default_endpoint);
        match default_ep.map(|ep| &ep.provider) {
            Some(provider) => load_instruction_prefix_for_provider(provider.clone()).await,
            None => None,
        }
    };
    let domain_cache = core
        .handles
        .cache
        .clone()
        .map(|h| DomainCacheHandle(Arc::new(h)));
    let extensions = AgentExtensions {
        cache: domain_cache,
        instruction_prefix,
        message_compactor: Some(
            augur_provider_openrouter::compaction::build_openrouter_message_compactor(),
        ),
    };
    actors::agent::agent_actor::spawn(
        AgentSpawnArgs::builder()
            .llm(core.handles.services.llm.clone())
            .tools(core.handles.services.tool.clone())
            .config(config.agent.clone())
            .services(
                AgentServices::builder()
                    .persistence(core.context.startup.persistence.clone())
                    .logger(core.handles.io.logger.clone())
                    .token_tracker(core.context.startup.token_tracker.clone())
                    .history_adapter(core.handles.io.history_adapter.clone())
                    .output_tx(core.context.control.agent_tx.clone())
                    .build(),
            )
            .runtime(
                AgentRuntime::builder()
                    .extensions(extensions)
                    .app_config(config.clone())
                    .request_cap_threshold(model_config.auto_compact_threshold)
                    .tool_response_cap(model_config.tool_response_cap)
                    .build(),
            )
            .build(),
    )
}

/// Load the instruction prefix for any provider from the provider catalog.
///
/// Returns `Some(Arc<InstructionPrefix>)` when the provider's catalog has
/// `instruction_files` configured and all (or some) files load successfully.
/// Returns `None` when the catalog is absent, the file list is empty, or
/// loading fails.
pub(super) async fn load_instruction_prefix_for_provider(
    provider: Provider,
) -> Option<Arc<augur_domain::domain::task_types::InstructionPrefix>> {
    use augur_domain::config::provider_catalog::default_provider_catalog_dir;
    use augur_domain::config::provider_catalog::load_provider_catalog;
    use augur_domain::domain::task_types::{InstructionFilePath, RepoRoot};
    use augur_provider_openrouter::actors::openrouter_task::instruction_loader::load_instruction_prefix;

    let catalog_dir = default_provider_catalog_dir();
    let catalog = load_provider_catalog(&catalog_dir, provider.clone())
        .ok()
        .flatten()?;
    let instruction_paths = catalog.instruction_files;
    if instruction_paths.is_empty() {
        return None;
    }
    let paths: Vec<InstructionFilePath> = instruction_paths
        .iter()
        .map(InstructionFilePath::new)
        .collect();
    let repo_root = RepoRoot::new(current_repo_root_string());
    match load_instruction_prefix(&paths, &repo_root).await {
        Ok(prefix) => Some(Arc::new(prefix)),
        Err(err) => {
            tracing::warn!(%err, provider = %provider, "failed to load instruction prefix");
            None
        }
    }
}

/// Load the OpenRouter instruction prefix from the provider catalog.
///
/// Delegates to [`load_instruction_prefix_for_provider`] with
/// [`Provider::OpenRouter`].
#[allow(dead_code)]
pub(super) async fn load_openrouter_instruction_prefix()
-> Option<Arc<augur_domain::domain::task_types::InstructionPrefix>> {
    load_instruction_prefix_for_provider(augur_domain::config::types::Provider::OpenRouter).await
}

fn current_repo_root_string() -> String {
    effective_repo_root().to_string_lossy().to_string()
}

/// Spawn the ask-agent actor and return an [`AskRuntime`].
///
/// Builds `\`AskSpawnArgs\`` with a fresh `PersistenceHandle` (scoped to the
/// sessions directory), then spawns the ask actor. After spawning, immediately
/// awaits `take_tool_join` so the tool join handle is captured before the
/// runtime handle is moved. Returns the actor join, tool join, and handle
/// wrapped in an [`AskRuntime`].
pub async fn spawn_ask_runtime(
    config: &AppConfig,
    program_settings: &ProgramSettings,
    core: &CoreRuntime,
) -> AskRuntime {
    let spawn_args = actors::ask::ask_actor::AskSpawnArgs::builder()
        .llm(core.handles.services.llm.clone())
        .config(config.agent.clone())
        .registry(
            actors::ask::ask_actor::AskRegistryConfig::builder()
                .file_read(core.handles.services.file_read.clone())
                .excluded_dirs(program_settings.excluded_directory_paths())
                .build(),
        )
        .runtime(
            actors::ask::ask_actor::AskRuntimeConfig::builder()
                .default_endpoint(config.default_endpoint.clone())
                .app_config(config.clone())
                .build(),
        )
        .services(
            AgentServices::builder()
                .persistence(PersistenceHandle::new(
                    core.context.startup.sessions_dir.clone(),
                ))
                .logger(core.handles.io.logger.clone())
                .token_tracker(core.context.startup.token_tracker.clone())
                .history_adapter(core.handles.io.history_adapter.clone())
                .build(),
        )
        .build();
    let (join, handle) = actors::ask::ask_actor::spawn(spawn_args);
    let tool_join = handle.take_tool_join().await;
    AskRuntime {
        join,
        tool_join,
        handle,
    }
}
