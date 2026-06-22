use augur_domain::config::types::{
    find_endpoint, AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials,
    PersistenceConfig, Provider,
};
use augur_domain::domain::{
    ApiKey, BearerToken, EndpointName, EndpointUrl, EnvVarName, FilePath, ModelName, OutputText,
    Temperature, TokenCount,
};
use augur_domain::domain::{NumericNewtype, StringNewtype};

fn make_config(names: &[&str]) -> AppConfig {
    let endpoints = names
        .iter()
        .map(|name| EndpointConfig {
            name: EndpointName::new(*name),
            provider: Provider::Ollama,
            base_url: EndpointUrl::new("http://localhost:11434"),
            model: ModelName::new("llama3.2"),
            credentials: EndpointCredentials::default(),
        })
        .collect();
    AppConfig {
        endpoints,
        default_endpoint: EndpointName::new(names[0]),
        agent: AgentConfig {
            system_prompt: OutputText::new("sys"),
            max_tokens: TokenCount::new(1024),
            temperature: Temperature::new(0.7),
            allowed_dirs: vec![],
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

#[test]
fn find_endpoint_returns_matching_entry() {
    let config = make_config(&["alpha", "beta"]);
    let found = find_endpoint(&config, &EndpointName::new("beta"));
    assert!(found.is_some());
    assert_eq!(found.expect("beta endpoint").name.as_str(), "beta");
}

#[test]
fn find_endpoint_unknown_name_returns_none() {
    let config = make_config(&["alpha", "beta"]);
    let found = find_endpoint(&config, &EndpointName::new("gamma"));
    assert!(found.is_none());
}

#[test]
fn find_endpoint_duplicate_names_returns_first_match() {
    let config = AppConfig {
        endpoints: vec![
            EndpointConfig {
                name: EndpointName::new("alpha"),
                provider: Provider::Ollama,
                base_url: EndpointUrl::new("http://first"),
                model: ModelName::new("llama3.2"),
                credentials: EndpointCredentials::default(),
            },
            EndpointConfig {
                name: EndpointName::new("alpha"),
                provider: Provider::Ollama,
                base_url: EndpointUrl::new("http://second"),
                model: ModelName::new("llama3.2"),
                credentials: EndpointCredentials::default(),
            },
        ],
        default_endpoint: EndpointName::new("alpha"),
        agent: AgentConfig {
            system_prompt: OutputText::new("sys"),
            max_tokens: TokenCount::new(1024),
            temperature: Temperature::new(0.7),
            allowed_dirs: vec![],
        },
        copilot: CopilotConfig::default(),
        persistence: PersistenceConfig {
            log_dir: FilePath::new("./logs"),
            sessions_dir: None,
        },
        program_settings: Default::default(),
        user_settings: Default::default(),
    };
    let found = find_endpoint(&config, &EndpointName::new("alpha")).expect("endpoint should exist");
    assert_eq!(found.base_url.as_str(), "http://first");
}

#[test]
fn provider_openrouter_deserializes_from_yaml_string() {
    let provider: Provider =
        serde_yaml::from_str("OpenRouter").expect("OpenRouter must deserialize");
    assert_eq!(provider, Provider::OpenRouter);
}

#[test]
fn provider_openrouter_round_trips_through_serde() {
    let serialized = serde_yaml::to_string(&Provider::OpenRouter).expect("serialize");
    let deserialized: Provider = serde_yaml::from_str(&serialized).expect("deserialize");
    assert_eq!(deserialized, Provider::OpenRouter);
}

#[test]
fn config_public_fields_use_wrapper_types() {
    let endpoint = EndpointConfig {
        name: EndpointName::new("ep"),
        provider: Provider::OpenRouter,
        base_url: EndpointUrl::new("https://openrouter.ai/api/v1"),
        model: ModelName::new("anthropic/claude-sonnet-4-5"),
        credentials: EndpointCredentials {
            api_key_env: Some(EnvVarName::new("OPENROUTER_API_KEY")),
            api_key: Some(ApiKey::new("sk-or-v1-test")),
        },
    };
    assert_eq!(
        endpoint.credentials.api_key_env,
        Some(EnvVarName::new("OPENROUTER_API_KEY"))
    );
    assert_eq!(
        endpoint.credentials.api_key,
        Some(ApiKey::new("sk-or-v1-test"))
    );

    let app = make_config(&["ep"]);
    assert_eq!(app.persistence.log_dir, FilePath::new("./logs"));

    let mut copilot = CopilotConfig::default();
    copilot.executor.sdk.cli_path = Some(FilePath::new("/usr/bin/gh"));
    copilot.executor.sdk.model = Some(ModelName::new("gpt-4o"));
    copilot.executor.sdk.auth_token = Some(BearerToken::new("executor-token"));
    assert_eq!(
        copilot.executor.sdk.auth_token,
        Some(BearerToken::new("executor-token"))
    );
}
