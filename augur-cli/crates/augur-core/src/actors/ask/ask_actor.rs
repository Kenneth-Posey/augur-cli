//! Ask actor: spawns a second AgentActor configured with a read-only tool registry.

use super::ask_actor_ops as actor_ops;
use super::handle::AskHandle;
use crate::actors::agent::agent_actor::{
    spawn as spawn_agent, AgentRuntime, AgentServices, AgentSpawnArgs,
};
use crate::actors::file_read::FileReadHandle;
use crate::actors::tool::tool_actor::spawn as spawn_tool;
use crate::tools::registry::ToolRegistry;
use augur_domain::config::types::AgentConfig;
use augur_domain::domain::string_newtypes::EndpointName;
use augur_domain::domain::task_types::AgentExtensions;
use augur_domain::domain::traits::LlmClient;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Arguments for spawning the ask-panel actor.
///
/// Bundles all inputs for the limited-capability ask agent. `services`
/// groups persistence, project settings, and logger. Stays within the
/// 5-field struct limit. No `cache` field - the ask panel never uses
/// proactive cache injection.
#[derive(bon::Builder)]
pub struct AskRegistryConfig {
    /// File-read handle shared with FileReadRangeTool and FileLineCountTool.
    pub file_read: FileReadHandle,
    /// Program-owned directory exclusions for the ask-panel list_directory tool.
    pub excluded_dirs: Vec<PathBuf>,
}

#[derive(bon::Builder)]
/// Runtime constants for the ask-panel actor.
pub struct AskRuntimeConfig {
    /// The default LLM endpoint this ask panel always submits to.
    ///
    /// Must be an endpoint from `config.endpoints` so the standard `LlmActor`
    /// can resolve it. Never set this to a Copilot endpoint name.
    pub default_endpoint: EndpointName,
    /// Application configuration for resolving endpoint definitions.
    pub app_config: augur_domain::config::types::AppConfig,
}

#[derive(bon::Builder)]
/// Spawn-time dependencies for the ask-panel agent actor.
///
/// Bundles the LLM client, ask-specific agent config, service handles, read-only
/// tool-registry inputs, and fixed default endpoint used for ask submissions.
pub struct AskSpawnArgs<L> {
    /// LLM client for sending ask-panel completion requests.
    pub llm: L,
    /// Agent behaviour configuration for the ask panel.
    pub config: AgentConfig,
    /// Supporting service handles (persistence, project settings, logger).
    pub services: AgentServices,
    /// Inputs used to build the read-only ask registry.
    pub registry: AskRegistryConfig,
    /// Runtime constants for endpoint selection and endpoint-resolution config.
    pub runtime: AskRuntimeConfig,
}

/// Build a [`ToolRegistry`] restricted to read-only operations.
///
/// Registers `file_read`, `file_read_range`, `file_line_count`, and
/// `list_directory`. Deliberately excludes `shell_exec`, `file_create`,
/// `query_user`, `set_working_file`, and `refresh_cache_file` to keep the
/// ask panel side-effect-free.
///
/// `allowed_dirs` is forwarded to `ListDirectoryTool` to enforce the same
/// sandbox restrictions as the main tool registry.
///
/// Units: none.
/// Consumers: `spawn` in this module; `build_ask_registry_*` tests.
pub(crate) fn build_ask_registry(
    file_read: FileReadHandle,
    allowed_dirs: Vec<PathBuf>,
    excluded_dirs: Vec<PathBuf>,
) -> ToolRegistry {
    actor_ops::build_ask_registry(file_read, allowed_dirs, excluded_dirs)
}

/// Spawn the ask-panel actor and return its join handle plus an `AskHandle`.
///
/// Builds the read-only `ToolRegistry` via `build_ask_registry`, spawns a
/// `ToolActor` with it, then spawns an `AgentActor` with no cache and the
/// limited tool handle. After spawning, calls `mark_as_ask_session` on the
/// persistence handle so all subsequent `save_turn` outputs carry
/// `ask_session: true` and are excluded from the session picker.
///
/// Returns `(agent_join, ask_handle)` where `agent_join` is the primary
/// lifecycle handle to await during shutdown. The tool actor's join handle
/// is stored inside `AskHandle` and retrievable via `take_tool_join()`.
///
/// Consumers: `wiring.rs` during actor construction.
pub fn spawn<L: LlmClient>(args: AskSpawnArgs<L>) -> (JoinHandle<()>, AskHandle) {
    let allowed_dirs = actor_ops::allowed_dirs_from_config(&args.config);
    let registry = build_ask_registry(
        args.registry.file_read,
        allowed_dirs,
        args.registry.excluded_dirs,
    );
    let (tool_join, tool_handle) = spawn_tool(registry);
    let ask_persistence = args.services.persistence.clone();
    let agent_args = AgentSpawnArgs::builder()
        .llm(args.llm)
        .tools(tool_handle)
        .config(args.config)
        .services(args.services)
        .runtime(
            AgentRuntime::builder()
                .extensions(AgentExtensions {
                    cache: None,
                    instruction_prefix: None,
                    message_compactor: None,
                })
                .app_config(args.runtime.app_config)
                .build(),
        )
        .build();
    let (agent_join, agent_handle) = spawn_agent(agent_args);
    ask_persistence.mark_as_ask_session();
    let handle = AskHandle::builder()
        .inner(agent_handle)
        .tool_join(Arc::new(Mutex::new(Some(tool_join))))
        .default_endpoint(args.runtime.default_endpoint)
        .build();
    (agent_join, handle)
}
