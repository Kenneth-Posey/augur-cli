use augur_core::actors::agent::agent_actor::{AgentRuntime, AgentServices, AgentSpawnArgs, spawn};
use augur_core::actors::agent::handle::AgentHandle;
use augur_core::actors::logger::logger_actor::spawn as spawn_logger;
use augur_core::helpers::fake_history_adapter::fake_history_adapter_handle;
use augur_core::helpers::fake_llm::FakeLlmClient;
use augur_core::helpers::fake_token_tracker::fake_token_tracker_handle;
use augur_core::helpers::fake_tool::FakeToolExecutor;
use augur_core::persistence::handle::PersistenceHandle;
use augur_domain::config::types::{AgentConfig, AppConfig, PersistenceConfig};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{EndpointName, FilePath, OutputText, StringNewtype};
use augur_domain::domain::task_types::AgentExtensions;

fn spawn_handle() -> AgentHandle {
    let dir = tempfile::tempdir().expect("tempdir");
    let persistence = PersistenceHandle::new(dir.path().to_owned());
    let log_dir = tempfile::tempdir().expect("log tempdir");
    let (_logger_join, logger) = spawn_logger(log_dir.path().to_path_buf());
    std::mem::forget(log_dir);

    let config = AgentConfig {
        system_prompt: OutputText::new("helpful"),
        max_tokens: TokenCount::new(1024),
        temperature: Temperature::new(0.5),
        allowed_dirs: vec![],
    };
    let args = AgentSpawnArgs::builder()
        .llm(FakeLlmClient::new(vec![]))
        .tools(FakeToolExecutor::always_ok(""))
        .config(config.clone())
        .services(
            AgentServices::builder()
                .persistence(persistence)
                .logger(logger)
                .token_tracker(fake_token_tracker_handle().1)
                .history_adapter(fake_history_adapter_handle())
                .build(),
        )
        .runtime(
            AgentRuntime::builder()
                .extensions(AgentExtensions {
                    cache: None,
                    instruction_prefix: None,
                    message_compactor: None,
                })
                .app_config(AppConfig {
                    endpoints: vec![],
                    default_endpoint: EndpointName::new("test"),
                    agent: config,
                    copilot: Default::default(),
                    persistence: PersistenceConfig {
                        log_dir: FilePath::new("./logs"),
                        sessions_dir: None,
                    },
                    program_settings: Default::default(),
                    user_settings: Default::default(),
                })
                .build(),
        )
        .build();
    let (_, handle) = spawn(args);
    handle
}

#[tokio::test]
async fn history_snapshot_returns_empty_when_no_turns() {
    let handle = spawn_handle();
    let history = handle.history_snapshot().await;
    assert!(history.is_empty());
}

#[tokio::test]
async fn get_state_defaults_when_no_turn_submitted() {
    let handle = spawn_handle();
    let state = handle.get_state().await;
    assert!(state.last_endpoint.is_none());
    assert!(state.selected_model.is_none());
}

#[tokio::test]
async fn interrupt_is_idempotent() {
    let handle = spawn_handle();
    handle.interrupt();
    handle.interrupt();
}

#[test]
fn legacy_interrupt_signal_visibility_is_crate_scoped() {
    let source = std::fs::read_to_string(format!(
        "{}/src/actors/agent/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("agent handle source must be readable");
    assert!(source.contains("pub(crate) fn is_cancelled(&self) -> CancelSignal"));
}
