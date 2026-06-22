use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::config::{AgentConfig, AppConfig, CopilotConfig, PersistenceConfig};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, FilePath, ModelName, OutputText,
};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{CompletionRequest, StringNewtype};
use augur_provider_openrouter::actors::llm::handle::LlmClient;
use augur_provider_openrouter::actors::llm::llm_actor;

fn test_app_config() -> AppConfig {
    AppConfig {
        endpoints: vec![EndpointConfig {
            name: EndpointName::new("default"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("http://localhost:1"),
            model: ModelName::new("openai/gpt-4o-mini"),
            credentials: EndpointCredentials::default(),
        }],
        default_endpoint: EndpointName::new("default"),
        agent: AgentConfig {
            system_prompt: OutputText::new(""),
            max_tokens: TokenCount::new(128),
            temperature: Temperature::new(0.5),
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

fn test_logger() -> augur_domain::domain::actor_contracts::LoggerHandle {
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    augur_domain::domain::actor_contracts::LoggerHandle::new(tx)
}

#[tokio::test]
async fn complete_stream_emits_error_when_endpoint_is_missing() {
    let (agent_tx, _agent_rx) = tokio::sync::broadcast::channel(8);
    let (join, handle) = llm_actor::spawn(test_app_config(), agent_tx, "test-session".to_string(), test_logger());
    let request = CompletionRequest::builder()
        .endpoint(EndpointName::new("missing"))
        .messages(vec![])
        .tools(vec![])
        .maybe_cache(None)
        .maybe_model_override(None)
        .build();

    let mut reply_rx = handle.complete_stream(request);
    let received = tokio::time::timeout(std::time::Duration::from_secs(2), reply_rx.recv())
        .await
        .expect("error message should arrive")
        .expect("channel should stay open long enough for one message");
    assert!(matches!(received, StreamChunk::Error(_)));

    handle.shutdown();
    let _ = join.await;
}
