//! Test helper: factory for a minimal `AskHandle` for use in TUI handle tests.

use crate::actors::agent::agent_actor::AgentServices;
use crate::actors::ask::ask_actor::{
    spawn as spawn_ask, AskRegistryConfig, AskRuntimeConfig, AskSpawnArgs,
};
use crate::actors::ask::AskHandle;
use crate::actors::file_read::file_read_actor::spawn as spawn_file_read;
use crate::actors::logger::logger_actor::spawn as spawn_logger;
use crate::persistence::handle::PersistenceHandle;
use augur_domain::config::types::{AgentConfig, PersistenceConfig};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{EndpointName, FilePath, OutputText, StringNewtype};

use super::fake_llm::FakeLlmClient;

/// Spawn a minimal ask actor and return its handle.
///
/// Use in TUI-related tests that construct `TuiToolHandles` or `TuiHandles`
/// and need an `AskHandle` to satisfy the type. The actor uses a `FakeLlmClient`
/// that returns empty responses. The returned `TempDir` keeps the persistence
/// directory alive for the test's duration - bind it to `_ask_dir`.
pub async fn make_ask_handle() -> (AskHandle, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir for ask handle");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let log_tmp = tempfile::tempdir().expect("log tempdir for ask handle");
    let (_logger_join, logger) = spawn_logger(log_tmp.path().to_path_buf());
    std::mem::forget(log_tmp);
    let (_file_join, file_read) = spawn_file_read(vec![]);
    let agent_config = AgentConfig {
        system_prompt: OutputText::new("test"),
        max_tokens: TokenCount::new(1024),
        temperature: Temperature::new(0.5),
        allowed_dirs: vec![],
    };
    let (_, handle) = spawn_ask(AskSpawnArgs {
        llm: FakeLlmClient::new(vec![]),
        config: agent_config.clone(),
        registry: AskRegistryConfig {
            file_read,
            excluded_dirs: vec![],
        },
        services: AgentServices::builder()
            .persistence(persistence)
            .logger(logger)
            .token_tracker(crate::helpers::fake_token_tracker::fake_token_tracker_handle().1)
            .history_adapter(crate::helpers::fake_history_adapter::fake_history_adapter_handle())
            .build(),
        runtime: AskRuntimeConfig {
            default_endpoint: EndpointName::new("test-ep"),
            app_config: augur_domain::config::types::AppConfig {
                endpoints: vec![],
                default_endpoint: EndpointName::new("test-ep"),
                agent: agent_config,
                copilot: Default::default(),
                persistence: PersistenceConfig {
                    log_dir: FilePath::new("./logs"),
                    sessions_dir: None,
                },
                program_settings: Default::default(),
                user_settings: Default::default(),
            },
        },
    });
    (handle, dir)
}
