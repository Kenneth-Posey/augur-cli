use augur_cli::wiring::{
    spawn_infrastructure, spawn_tui_runtime, spawn_tui_sub_actors, take_query_rx,
};
use augur_domain::config::types::{
    AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials, PersistenceConfig,
    ProgramSettings, Provider,
};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TimestampSecs, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, FilePath, ModelName, OutputText,
};
use augur_domain::domain::types::StreamChunk;
use augur_domain::domain::StringNewtype;

fn test_config() -> AppConfig {
    AppConfig {
        endpoints: vec![EndpointConfig {
            name: EndpointName::new("openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("https://openrouter.ai/api/v1"),
            model: ModelName::new("openai/gpt-4.1-mini"),
            credentials: EndpointCredentials::default(),
        }],
        default_endpoint: EndpointName::new("openrouter"),
        agent: AgentConfig {
            system_prompt: OutputText::new("sys"),
            max_tokens: TokenCount::new(1024),
            temperature: Temperature::new(0.5),
            allowed_dirs: vec![FilePath::new(".")],
        },
        copilot: CopilotConfig::default(),
        persistence: PersistenceConfig {
            log_dir: FilePath::new("./logs"),
            sessions_dir: Some(FilePath::new(
                std::env::temp_dir()
                    .join("augur-tui-wiring-tests")
                    .to_str()
                    .unwrap_or("/tmp/augur-tui-wiring-tests"),
            )),
        },
        program_settings: Default::default(),
        user_settings: Default::default(),
    }
}

/// Test that TUI runtime functions are accessible
#[test]
fn spawn_tui_runtime_accessible() {
    let function_name = core::any::type_name_of_val(&spawn_tui_runtime);
    assert!(function_name.contains("spawn_tui_runtime"));
}

/// Test that take_query_rx is accessible
#[test]
fn take_query_rx_accessible() {
    let function_name = core::any::type_name_of_val(&take_query_rx);
    assert!(function_name.contains("take_query_rx"));
}

#[tokio::test]
async fn take_query_rx_returns_live_then_closed_receiver() {
    let mut core = spawn_infrastructure(
        &test_config(),
        &ProgramSettings::default(),
        TimestampSecs::new(1),
    );
    let mut first = take_query_rx(&mut core);
    assert!(matches!(
        first.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
    let mut second = take_query_rx(&mut core);
    assert!(matches!(
        second.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
    ));
}

#[tokio::test]
async fn spawn_tui_sub_actors_initializes_handles() {
    let handles = spawn_tui_sub_actors();
    assert_eq!(handles.agent_panel.current_state().output.len(), 0);
    assert_eq!(handles.main_feed.current_state().lines.len(), 0);
    handles.agent_panel.shutdown();
    handles.main_feed.shutdown();
    handles.ask_panel.shutdown();
    handles.overlays.chat_menu.shutdown();
    handles.overlays.spinner.shutdown();
    handles.overlays.controls.shutdown();
}

#[tokio::test]
async fn spawn_consumer_actors_bridges_user_chunk_and_error_to_main_feed() {
    let sub = spawn_tui_sub_actors();
    let consumers =
        augur_cli::wiring::spawn_consumer_actors(sub.main_feed.clone(), sub.agent_panel.clone());
    consumers
        .llm_feed
        .consume(StreamChunk::Token(OutputText::new("bg token")));
    consumers
        .llm_feed
        .consume(StreamChunk::Error(OutputText::new("bg error")));

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(250);
    loop {
        let lines = sub.main_feed.current_state().lines;
        if lines.len() >= 2 {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    consumers.llm_feed.shutdown();
    consumers.user_message.shutdown();
    sub.agent_panel.shutdown();
    sub.main_feed.shutdown();
    sub.ask_panel.shutdown();
    sub.overlays.chat_menu.shutdown();
    sub.overlays.spinner.shutdown();
    sub.overlays.controls.shutdown();
}
