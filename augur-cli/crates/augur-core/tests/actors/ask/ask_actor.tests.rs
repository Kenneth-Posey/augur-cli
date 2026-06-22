use augur_core::actors::agent::agent_actor::AgentServices;
use augur_core::actors::agent::agent_ops::AgentOutput;
use augur_core::actors::ask::ask_actor::{
    spawn, AskRegistryConfig, AskRuntimeConfig, AskSpawnArgs,
};
use augur_core::actors::file_read::file_read_actor::spawn as spawn_file_read;
use augur_core::actors::logger::logger_actor::spawn as spawn_logger;
use augur_core::helpers::fake_history_adapter::fake_history_adapter_handle;
use augur_core::helpers::fake_llm::FakeLlmClient;
use augur_core::helpers::fake_token_tracker::fake_token_tracker_handle;
use augur_core::persistence::handle::PersistenceHandle;
use augur_core::persistence::store;
use augur_domain::config::types::{AgentConfig, AppConfig, PersistenceConfig};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, FilePath, OutputText, PromptText, StringNewtype,
};
use augur_domain::domain::types::StreamChunk;
use std::time::Duration;
use tempfile::TempDir;

fn make_file_read() -> (
    augur_core::actors::file_read::FileReadHandle,
    tokio::task::JoinHandle<()>,
) {
    let (join, handle) = spawn_file_read(vec![]);
    (handle, join)
}

fn make_persistence() -> (PersistenceHandle, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let handle = PersistenceHandle::new(dir.path().to_owned());
    (handle, dir)
}

fn make_services(persistence: PersistenceHandle) -> AgentServices {
    let tmp = tempfile::tempdir().expect("tempdir for logger");
    let (_logger_join, logger) = spawn_logger(tmp.path().to_path_buf());
    std::mem::forget(tmp);
    AgentServices::builder()
        .persistence(persistence)
        .logger(logger)
        .token_tracker(fake_token_tracker_handle().1)
        .history_adapter(fake_history_adapter_handle())
        .build()
}

fn test_config() -> AgentConfig {
    AgentConfig {
        system_prompt: OutputText::new("You are a helpful read-only assistant."),
        max_tokens: TokenCount::new(4096),
        temperature: Temperature::new(0.7),
        allowed_dirs: vec![],
    }
}

fn app_config() -> AppConfig {
    AppConfig {
        endpoints: vec![],
        default_endpoint: EndpointName::new("test-ep"),
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

#[tokio::test]
async fn spawn_and_shutdown() {
    let (file_read, _fr_join) = make_file_read();
    let (persistence, _dir) = make_persistence();
    let args = AskSpawnArgs::builder()
        .llm(FakeLlmClient::new(vec![]))
        .config(test_config())
        .services(make_services(persistence))
        .registry(
            AskRegistryConfig::builder()
                .file_read(file_read)
                .excluded_dirs(vec![])
                .build(),
        )
        .runtime(
            AskRuntimeConfig::builder()
                .default_endpoint(EndpointName::new("test-ep"))
                .app_config(app_config())
                .build(),
        )
        .build();
    let (join, handle) = spawn(args);
    handle.shutdown();
    join.await.expect("ask actor task must not panic");
}

#[tokio::test]
async fn spawn_marks_session_as_ask_in_persistence() {
    let (file_read, _fr_join) = make_file_read();
    let (persistence, dir) = make_persistence();
    let check = persistence.clone();
    let llm = FakeLlmClient::new(vec![vec![
        StreamChunk::Token(OutputText::new("hi")),
        StreamChunk::Done,
    ]]);
    let args = AskSpawnArgs::builder()
        .llm(llm)
        .config(test_config())
        .services(make_services(persistence))
        .registry(
            AskRegistryConfig::builder()
                .file_read(file_read)
                .excluded_dirs(vec![])
                .build(),
        )
        .runtime(
            AskRuntimeConfig::builder()
                .default_endpoint(EndpointName::new("test-ep"))
                .app_config(app_config())
                .build(),
        )
        .build();
    let (join, handle) = spawn(args);

    let mut rx = handle.subscribe_output();
    handle.submit(PromptText::new("q"));
    let deadline = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            result = rx.recv() => match result {
                Ok(AgentOutput::Done) => break,
                Err(_) => break,
                _ => {}
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.shutdown();
    join.await.expect("ask actor task must not panic");

    let loaded = store::load_session(dir.path(), &check.session_id()).expect("load session");
    assert!(loaded.meta.flags.ask_session.0);
}

#[tokio::test]
async fn submit_yields_tokens_then_done() {
    let (file_read, _fr_join) = make_file_read();
    let (persistence, _dir) = make_persistence();
    let llm = FakeLlmClient::new(vec![vec![
        StreamChunk::Token(OutputText::new("hello")),
        StreamChunk::Done,
    ]]);
    let args = AskSpawnArgs::builder()
        .llm(llm)
        .config(test_config())
        .services(make_services(persistence))
        .registry(
            AskRegistryConfig::builder()
                .file_read(file_read)
                .excluded_dirs(vec![])
                .build(),
        )
        .runtime(
            AskRuntimeConfig::builder()
                .default_endpoint(EndpointName::new("test-ep"))
                .app_config(app_config())
                .build(),
        )
        .build();
    let (join, handle) = spawn(args);

    let mut rx = handle.subscribe_output();
    handle.submit(PromptText::new("test question"));
    let mut saw_token = false;
    let deadline = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            result = rx.recv() => match result {
                Ok(AgentOutput::Token(t)) if t.as_str() == "hello" => saw_token = true,
                Ok(AgentOutput::Done) => break,
                Err(_) => break,
                _ => {}
            }
        }
    }
    handle.shutdown();
    join.await.expect("ask actor task must not panic");
    assert!(saw_token);
}

#[test]
fn legacy_build_ask_registry_tests_deprecated_due_crate_visibility() {
    let source = std::fs::read_to_string(format!(
        "{}/src/actors/ask/ask_actor.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("ask actor source must be readable");
    assert!(source.contains("pub(crate) fn build_ask_registry("));
}
