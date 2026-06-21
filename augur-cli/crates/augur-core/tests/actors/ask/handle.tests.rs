use augur_core::actors::agent::agent_actor::AgentServices;
use augur_core::actors::ask::ask_actor::{
    spawn, AskRegistryConfig, AskRuntimeConfig, AskSpawnArgs,
};
use augur_core::actors::ask::handle::AskHandle;
use augur_core::actors::file_read::file_read_actor::spawn as spawn_file_read;
use augur_core::actors::logger::logger_actor::spawn as spawn_logger;
use augur_core::helpers::fake_history_adapter::fake_history_adapter_handle;
use augur_core::helpers::fake_llm::FakeLlmClient;
use augur_core::helpers::fake_token_tracker::fake_token_tracker_handle;
use augur_core::persistence::handle::PersistenceHandle;
use augur_domain::config::types::{AgentConfig, AppConfig, PersistenceConfig};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{EndpointName, FilePath, OutputText, StringNewtype};

fn test_config() -> AgentConfig {
    AgentConfig {
        system_prompt: OutputText::new("You are a helpful read-only assistant."),
        max_tokens: TokenCount::new(4096),
        temperature: Temperature::new(0.7),
        allowed_dirs: vec![],
    }
}

fn make_services() -> (AgentServices, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let log_dir = tempfile::tempdir().expect("log tempdir");
    let (_logger_join, logger) = spawn_logger(log_dir.path().to_path_buf());
    std::mem::forget(log_dir);
    (
        AgentServices::builder()
            .persistence(persistence)
            .logger(logger)
            .token_tracker(fake_token_tracker_handle().1)
            .history_adapter(fake_history_adapter_handle())
            .build(),
        dir,
    )
}

fn app_config() -> AppConfig {
    AppConfig {
        endpoints: vec![],
        default_endpoint: EndpointName::new("test"),
        agent: test_config(),
        copilot: Default::default(),
        persistence: PersistenceConfig {
            log_dir: FilePath::new("./logs"),
            sessions_dir: None,
        },
        program_settings: Default::default(),
        user_settings: Default::default(),
    }
}

async fn spawn_handle() -> AskHandle {
    let (_file_join, file_read) = spawn_file_read(vec![]);
    let (services, _dir) = make_services();
    let (_join, handle) = spawn(
        AskSpawnArgs::builder()
            .llm(FakeLlmClient::new(vec![]))
            .config(test_config())
            .services(services)
            .registry(
                AskRegistryConfig::builder()
                    .file_read(file_read)
                    .excluded_dirs(vec![])
                    .build(),
            )
            .runtime(
                AskRuntimeConfig::builder()
                    .default_endpoint(EndpointName::new("ask-endpoint"))
                    .app_config(app_config())
                    .build(),
            )
            .build(),
    );
    handle
}

#[tokio::test]
async fn default_endpoint_returns_configured_endpoint() {
    let handle = spawn_handle().await;
    assert_eq!(handle.default_endpoint().as_str(), "ask-endpoint");
    handle.shutdown();
}

#[tokio::test]
async fn take_tool_join_returns_some_once() {
    let handle = spawn_handle().await;
    assert!(handle.take_tool_join().await.is_some());
    assert!(handle.take_tool_join().await.is_none());
    handle.shutdown();
}
