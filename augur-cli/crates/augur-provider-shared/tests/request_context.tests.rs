use augur_domain::config::types::{
    AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials, PersistenceConfig,
    Provider,
};
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    ApiKey, EndpointName, EndpointUrl, EnvVarName, FilePath, ModelId, ModelName, OutputText,
};
use augur_domain::domain::types::Message;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_shared::request_context::{
    build_request_context, resolve_api_key, CompleteFields, CompleteRoute, LlmError, RequestPayload,
};

fn test_app_config(endpoint: EndpointConfig) -> AppConfig {
    AppConfig {
        endpoints: vec![endpoint],
        default_endpoint: EndpointName::new("test-endpoint"),
        agent: AgentConfig {
            system_prompt: OutputText::new(""),
            max_tokens: TokenCount::new(64),
            temperature: Temperature::new(0.25),
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

fn test_endpoint(credentials: EndpointCredentials) -> EndpointConfig {
    EndpointConfig {
        name: EndpointName::new("test-endpoint"),
        provider: Provider::OpenRouter,
        base_url: EndpointUrl::new("https://example.invalid"),
        model: ModelName::new("test-model"),
        credentials,
    }
}

#[test]
fn resolve_api_key_returns_direct_key() {
    let endpoint = test_endpoint(EndpointCredentials {
        api_key_env: None,
        api_key: Some(ApiKey::new("direct-key")),
    });

    let key = resolve_api_key(&endpoint).expect("direct key should resolve");

    assert_eq!(&*key, "direct-key");
}

#[test]
fn resolve_api_key_returns_empty_for_unauthenticated_endpoint() {
    let endpoint = test_endpoint(EndpointCredentials::default());

    let key = resolve_api_key(&endpoint).expect("empty key should resolve");

    assert!(key.is_empty());
}

#[test]
fn resolve_api_key_reads_env_key() {
    let name = format!("COPILOT_TEST_API_KEY_{}", std::process::id());
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var(&name, "env-key") };
    let endpoint = test_endpoint(EndpointCredentials {
        api_key_env: Some(EnvVarName::new(&name)),
        api_key: None,
    });

    let key = resolve_api_key(&endpoint).expect("env key should resolve");

    assert_eq!(&*key, "env-key");
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::remove_var(name) };
}

#[test]
fn build_request_context_applies_model_override() {
    let endpoint = test_endpoint(EndpointCredentials {
        api_key_env: None,
        api_key: Some(ApiKey::new("direct-key")),
    });
    let config = test_app_config(endpoint.clone());
    let (reply_tx, _reply_rx) = tokio::sync::mpsc::channel(1);
    let fields = CompleteFields::builder()
        .route(
            CompleteRoute::builder()
                .endpoint(EndpointName::new("test-endpoint"))
                .maybe_model_override(Some(ModelId::new("override-model")))
                .build(),
        )
        .payload(
            RequestPayload::builder()
                .messages(vec![Message::user("hello")])
                .tools(vec![])
                .build(),
        )
        .reply_tx(reply_tx)
        .build();

    let ctx = build_request_context(fields, &config).expect("request context should build");

    assert_eq!(&*ctx.endpoint.model, "override-model");
    assert_eq!(ctx.payload.messages.len(), 1);
    assert_eq!(ctx.params.max_tokens, TokenCount::new(64));
}

#[test]
fn build_request_context_rejects_unknown_endpoint() {
    let endpoint = test_endpoint(EndpointCredentials {
        api_key_env: None,
        api_key: Some(ApiKey::new("direct-key")),
    });
    let config = test_app_config(endpoint);
    let (reply_tx, _reply_rx) = tokio::sync::mpsc::channel(1);
    let fields = CompleteFields::builder()
        .route(
            CompleteRoute::builder()
                .endpoint(EndpointName::new("missing"))
                .build(),
        )
        .payload(
            RequestPayload::builder()
                .messages(vec![])
                .tools(vec![])
                .build(),
        )
        .reply_tx(reply_tx)
        .build();

    let result = build_request_context(fields, &config);

    assert!(matches!(result, Err(LlmError::UnknownEndpoint(_))));
}
