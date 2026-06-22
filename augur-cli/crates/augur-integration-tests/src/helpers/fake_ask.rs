//! Test helper: factory for a minimal `AskHandle` for use in TUI handle tests.

use crate::actors::agent::agent_actor::AgentServices;
use crate::actors::ask::ask_actor::{spawn as spawn_ask, AskRegistryConfig, AskSpawnArgs};
use crate::actors::ask::AskHandle;
use crate::actors::file_read::file_read_actor::spawn as spawn_file_read;
use crate::actors::logger::logger_actor::spawn as spawn_logger;
use augur_domain::config::types::AgentConfig;
use crate::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use crate::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use crate::persistence::handle::PersistenceHandle;

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
    let (_, handle) = spawn_ask(AskSpawnArgs {
        llm: FakeLlmClient::new(vec![]),
        config: AgentConfig {
            system_prompt: OutputText::new("test"),
            max_tokens: TokenCount::new(1024),
            temperature: Temperature::new(0.5),
            allowed_dirs: vec![],
        },
        registry: AskRegistryConfig {
            file_read,
            excluded_dirs: vec![],
        },
        default_endpoint: EndpointName::new("test-ep"),
        app_config: crate::config::AppConfig {
            endpoints: vec![],
            default_endpoint: EndpointName::new("test"),
            agent: AgentConfig {
                system_prompt: OutputText::new("test"),
                max_tokens: TokenCount::new(1024),
                temperature: Temperature::new(0.5),
                allowed_dirs: vec![],
            },
            copilot: Default::default(),
            persistence: crate::config::PersistenceConfig {
                log_dir: crate::domain::string_newtypes::FilePath::new("./logs"),
                sessions_dir: None,
            },
                program_settings: Default::default(),
                user_settings: Default::default(),
        },
        services: AgentServices::builder()
            .persistence(persistence)
            .logger(logger)
            .token_tracker(crate::tests::helpers::fake_token_tracker::fake_token_tracker_handle().1)
            .history_adapter(
                crate::tests::helpers::fake_history_adapter::fake_history_adapter_handle(),
            )
            .build(),
    });
    (handle, dir)
}
