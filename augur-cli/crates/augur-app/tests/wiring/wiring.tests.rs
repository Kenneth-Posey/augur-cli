use augur_cli::wiring::spawn_infrastructure;
use augur_domain::config::types::{AppConfig, ProgramSettings};
use augur_domain::domain::newtypes::TimestampSecs;
use augur_domain::domain::{NumericNewtype, StringNewtype};

/// Test that spawn_infrastructure works with basic configuration
#[tokio::test]
async fn spawn_infrastructure_returns_runtime() {
    let config = minimal_app_config();
    let program_settings = ProgramSettings::default();
    let _core = spawn_infrastructure(&config, &program_settings, TimestampSecs::new(1));
    // If we get here without panicking, the test passes
}

fn minimal_app_config() -> AppConfig {
    use augur_domain::config::types::{
        CopilotConfig, EndpointConfig, EndpointCredentials, PersistenceConfig, Provider,
    };
    use augur_domain::domain::newtypes::{Temperature, TokenCount};
    use augur_domain::domain::string_newtypes::{
        EndpointName, EndpointUrl, FilePath, ModelName, OutputText,
    };

    AppConfig {
        endpoints: vec![EndpointConfig {
            name: EndpointName::new("openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("https://openrouter.ai/api/v1"),
            model: ModelName::new("openai/gpt-4-mini"),
            credentials: EndpointCredentials::default(),
        }],
        default_endpoint: EndpointName::new("openrouter"),
        agent: augur_domain::config::types::AgentConfig {
            system_prompt: OutputText::new("system"),
            max_tokens: TokenCount::new(2048),
            temperature: Temperature::new(0.7),
            allowed_dirs: vec![FilePath::new(".")],
        },
        copilot: CopilotConfig::default(),
        persistence: PersistenceConfig {
            log_dir: FilePath::new("./logs"),
            sessions_dir: Some(FilePath::new(
                std::env::temp_dir()
                    .join("augur-cli-wiring-tests")
                    .to_str()
                    .unwrap_or("/tmp/augur-cli-wiring-tests"),
            )),
        },
        program_settings: Default::default(),
        user_settings: Default::default(),
    }
}
