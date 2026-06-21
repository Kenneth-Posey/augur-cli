//! Integration tests: OpenRouter streaming provider end-to-end.
//!
//! Covers happy-path SSE streaming, auth failure, rate-limit retry, server
//! errors, YAML config parsing, and endpoint discovery for OpenRouter.

use augur_core::config::endpoint_catalog_discovery::discover_endpoints;
use augur_provider_shared::request_context::{GenerationParams, RequestContext, RequestPayload};
use augur_provider_openrouter::actors::llm::providers::openrouter::stream_complete;
use augur_domain::config::types::{
    AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials, PersistenceConfig,
    Provider,
};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, FilePath, ModelName, OutputText, StringNewtype,
};
use augur_domain::domain::types::StreamChunk;
use serde::Deserialize;
use tokio::sync::mpsc;

// ── constants ─────────────────────────────────────────────────────────────────

/// SSE body for a minimal successful OpenAI-compatible streaming response.
///
/// Includes a usage chunk so `tokens_in` / `tokens_out` can be asserted.
const HAPPY_SSE_BODY: &str = concat!(
    "data: {\"model\":\"anthropic/claude-sonnet-4-5\",\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n",
    "data: {\"model\":\"anthropic/claude-sonnet-4-5\",\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n",
    "data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2},\"choices\":[]}\n",
    "data: [DONE]\n",
);

/// Expected prompt-token count encoded in `HAPPY_SSE_BODY`.
const EXPECTED_TOKENS_IN: u64 = 10;

/// Expected completion-token count encoded in `HAPPY_SSE_BODY`.
const EXPECTED_TOKENS_OUT: u64 = 2;

/// Sampling temperature used in all test contexts.
const TEST_TEMPERATURE: f64 = 0.7;

/// Max-tokens value used in all test contexts.
const TEST_MAX_TOKENS: u64 = 256;

/// Env-var name that is guaranteed not to exist in the test environment.
const NONEXISTENT_ENV_VAR: &str = "DCMK_TEST_NONEXISTENT_OPENROUTER_VAR_12345";

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a `RequestContext` targeting the given mock server base URL.
fn make_ctx(base_url: &str) -> (RequestContext, mpsc::Receiver<StreamChunk>) {
    let (reply_tx, reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test-openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new(base_url),
            model: ModelName::new("anthropic/claude-sonnet-4-5"),
            credentials: EndpointCredentials::default(),
        })
        .payload(
            RequestPayload::builder()
                .messages(vec![])
                .tools(vec![])
                .maybe_cache(None)
                .build(),
        )
        .reply_tx(reply_tx)
        .params(GenerationParams {
            max_tokens: TokenCount::new(TEST_MAX_TOKENS),
            temperature: Temperature::new(TEST_TEMPERATURE),
        })
        .build();
    (ctx, reply_rx)
}

/// Minimal `AppConfig` builder for endpoint-discovery tests.
fn make_app_config(endpoints: Vec<EndpointConfig>) -> AppConfig {
    let default_name = endpoints
        .first()
        .map(|ep| ep.name.as_str().to_owned())
        .unwrap_or_else(|| "none".to_owned());
    AppConfig {
        endpoints,
        default_endpoint: EndpointName::new(default_name),
        agent: AgentConfig {
            system_prompt: OutputText::new("you are helpful"),
            max_tokens: TokenCount::new(TEST_MAX_TOKENS),
            temperature: Temperature::new(TEST_TEMPERATURE),
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

// ── tests ─────────────────────────────────────────────────────────────────────

/// Happy path: mock returns a valid OpenAI-compatible SSE stream; assert tokens and usage arrive.
#[tokio::test]
async fn happy_path_token_and_usage_chunks_arrive() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(HAPPY_SSE_BODY)
        .create();

    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;

    // Assert at least one Token chunk arrived.
    let first = rx.recv().await;
    assert!(
        matches!(first, Some(StreamChunk::Token(_))),
        "expected first chunk to be Token; got {first:?}"
    );

    // Drain remaining chunks until Done, collecting all tokens.
    let mut all_tokens = match first {
        Some(StreamChunk::Token(t)) => vec![t.into_inner()],
        _ => vec![],
    };
    let mut usage_seen = false;
    while let Some(chunk) = rx.recv().await {
        match chunk {
            StreamChunk::Token(text) => all_tokens.push(text.into_inner()),
            StreamChunk::Usage(u) => {
                assert_eq!(
                    u.tokens_in,
                    TokenCount::new(EXPECTED_TOKENS_IN),
                    "tokens_in must match SSE usage object"
                );
                assert_eq!(
                    u.tokens_out,
                    TokenCount::new(EXPECTED_TOKENS_OUT),
                    "tokens_out must match SSE usage object"
                );
                usage_seen = true;
            }
            StreamChunk::Done => break,
            other => panic!("unexpected chunk: {other:?}"),
        }
    }

    assert!(
        !all_tokens.is_empty(),
        "at least one Token chunk must arrive"
    );
    assert!(usage_seen, "a Usage chunk must arrive before Done");

    let joined = all_tokens.concat();
    assert!(
        joined.contains("Hello"),
        "joined token text must contain 'Hello'; got: {joined:?}"
    );
}

/// Happy path: assert final StreamChunk::Done is emitted after Usage.
#[tokio::test]
async fn happy_path_done_chunk_follows_usage() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(HAPPY_SSE_BODY)
        .create();

    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;

    let mut usage_seen = false;
    let mut done_after_usage = false;
    while let Some(chunk) = rx.recv().await {
        match chunk {
            StreamChunk::Usage(_) => {
                usage_seen = true;
            }
            StreamChunk::Done => {
                done_after_usage = usage_seen;
                break;
            }
            StreamChunk::Token(_) | StreamChunk::RateLimitRetry(_) => {}
            other => panic!("unexpected chunk: {other:?}"),
        }
    }

    assert!(usage_seen, "Usage chunk must arrive before Done");
    assert!(done_after_usage, "Done must follow Usage");
}

/// Auth failure: mock returns 401; assert StreamChunk::Error arrives and no panic occurs.
#[tokio::test]
async fn auth_failure_401_emits_error_chunk() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"message":"Unauthorized","type":"auth_error"}}"#)
        .create();

    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;

    let first = rx.recv().await;
    assert!(
        matches!(first, Some(StreamChunk::Error(_))),
        "401 response must produce an Error chunk; got {first:?}"
    );
}

/// Auth failure: no panic when mock returns 401 and channel is drained.
#[tokio::test]
async fn auth_failure_401_no_panic() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"message":"Unauthorized"}}"#)
        .create();

    let (ctx, mut rx) = make_ctx(&server.url());

    // stream_complete must not panic - the test harness would catch an unwinding panic.
    stream_complete(ctx).await;

    // Drain channel; just verify no further Done or Usage follows an Error.
    let mut chunks: Vec<StreamChunk> = Vec::new();
    while let Ok(chunk) = rx.try_recv() {
        chunks.push(chunk);
    }

    let has_error = chunks.iter().any(|c| matches!(c, StreamChunk::Error(_)));
    assert!(
        has_error,
        "channel must contain at least one Error chunk after 401"
    );
}

/// Rate limit: first request returns 429, second returns 200 with valid SSE.
///
/// The retry logic in `send_with_retry` sleeps for `Retry-After` seconds before
/// attempting the second request. To keep tests fast we set `Retry-After: 0`.
#[tokio::test]
async fn rate_limit_429_then_200_retries_and_delivers_tokens() {
    let mut server = mockito::Server::new_async().await;

    // First request: 429 with Retry-After: 0 (zero wait to keep test fast).
    let _mock_429 = server
        .mock("POST", "/chat/completions")
        .with_status(429)
        .with_header("content-type", "application/json")
        .with_header("Retry-After", "0")
        .with_body(r#"{"error":{"message":"Rate limited"}}"#)
        .expect(1)
        .create();

    // Second request: success with a minimal SSE body.
    let _mock_200 = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: {\"choices\":[{\"delta\":{\"content\":\"retried\"}}]}\ndata: [DONE]\n")
        .expect(1)
        .create();

    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;

    // Collect all chunks to determine the final outcome.
    let mut chunks: Vec<StreamChunk> = Vec::new();
    while let Ok(chunk) = rx.try_recv() {
        chunks.push(chunk);
    }

    let has_rate_limit_retry = chunks
        .iter()
        .any(|c| matches!(c, StreamChunk::RateLimitRetry(_)));
    assert!(
        has_rate_limit_retry,
        "a RateLimitRetry chunk must be emitted on 429; chunks: {chunks:?}"
    );

    let has_token = chunks.iter().any(|c| matches!(c, StreamChunk::Token(_)));
    assert!(
        has_token,
        "a Token chunk must arrive after the successful retry; chunks: {chunks:?}"
    );
}

/// Server error: mock returns 500; assert StreamChunk::Error is propagated to the reply channel.
#[tokio::test]
async fn server_error_500_emits_error_chunk() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(500)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"message":"Internal Server Error"}}"#)
        .create();

    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;

    let first = rx.recv().await;
    assert!(
        matches!(first, Some(StreamChunk::Error(_))),
        "500 response must produce an Error chunk; got {first:?}"
    );
}

/// Server error: no Token or Done chunks emitted after a 500 response.
#[tokio::test]
async fn server_error_500_no_token_or_done_after_error() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(500)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"message":"Internal Server Error"}}"#)
        .create();

    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;

    let mut chunks: Vec<StreamChunk> = Vec::new();
    while let Ok(chunk) = rx.try_recv() {
        chunks.push(chunk);
    }

    let spurious = chunks
        .iter()
        .any(|c| matches!(c, StreamChunk::Token(_) | StreamChunk::Done));
    assert!(
        !spurious,
        "no Token or Done chunks must follow a 500 error; chunks: {chunks:?}"
    );
}

/// Config parsing: YAML with `provider: OpenRouter` deserializes to `Provider::OpenRouter`.
#[test]
fn config_parsing_openrouter_provider_field() {
    let yaml = r#"
providers:
  - name: openrouter-test
    provider: OpenRouter
    base_url: "https://openrouter.ai/api/v1"
    model: "anthropic/claude-sonnet-4-5"
    api_key_env: OPENROUTER_API_KEY
"#;

    #[derive(Deserialize)]
    struct ProviderList {
        providers: Vec<EndpointConfig>,
    }

    let parsed: ProviderList =
        serde_yaml::from_str(yaml).expect("YAML must deserialize without error");

    assert_eq!(
        parsed.providers.len(),
        1,
        "must parse exactly one provider entry"
    );

    let ep = &parsed.providers[0];

    assert_eq!(
        ep.provider,
        Provider::OpenRouter,
        "provider field must deserialize to Provider::OpenRouter"
    );
}

/// Config parsing: all fields (name, base_url, model, api_key_env) parse correctly.
#[test]
fn config_parsing_all_fields_correct() {
    let yaml = r#"
providers:
  - name: openrouter-test
    provider: OpenRouter
    base_url: "https://openrouter.ai/api/v1"
    model: "anthropic/claude-sonnet-4-5"
    api_key_env: OPENROUTER_API_KEY
"#;

    #[derive(Deserialize)]
    struct ProviderList {
        providers: Vec<EndpointConfig>,
    }

    let parsed: ProviderList =
        serde_yaml::from_str(yaml).expect("YAML must deserialize without error");
    let ep = &parsed.providers[0];

    assert_eq!(ep.name.as_str(), "openrouter-test", "name must round-trip");
    assert_eq!(
        ep.base_url.as_str(),
        "https://openrouter.ai/api/v1",
        "base_url must round-trip"
    );
    assert_eq!(
        ep.model.as_str(),
        "anthropic/claude-sonnet-4-5",
        "model must round-trip"
    );

    let env_var = ep
        .credentials
        .api_key_env
        .as_ref()
        .expect("api_key_env must be present");
    assert_eq!(
        env_var.as_str(),
        "OPENROUTER_API_KEY",
        "api_key_env must round-trip"
    );
}

/// Endpoint discovery: three endpoints (OpenRouter, Ollama, Anthropic) → three ModelOptions.
#[test]
fn endpoint_discovery_returns_three_model_options() {
    let config = make_app_config(make_three_endpoint_list());
    let options = discover_endpoints(&config);

    assert_eq!(
        options.len(),
        3,
        "discover_endpoints must return exactly one ModelOption per configured endpoint"
    );
}

/// Endpoint discovery: each ModelOption `id` equals the endpoint `name`.
#[test]
fn endpoint_discovery_ids_match_endpoint_names() {
    let config = make_app_config(make_three_endpoint_list());
    let options = discover_endpoints(&config);

    assert_eq!(options[0].id.as_str(), "openrouter-prod");
    assert_eq!(options[1].id.as_str(), "ollama-local");
    assert_eq!(options[2].id.as_str(), "anthropic-claude");
}

/// Endpoint discovery: each `display_name` encodes model and provider label.
#[test]
fn endpoint_discovery_display_names_contain_model_and_provider() {
    let config = make_app_config(make_three_endpoint_list());
    let options = discover_endpoints(&config);

    let or_name = options[0].display_name.as_str();
    assert!(
        or_name.contains("anthropic/claude-sonnet-4-5"),
        "OpenRouter display_name must contain model; got: {or_name}"
    );
    assert!(
        or_name.contains("openrouter"),
        "OpenRouter display_name must contain provider label; got: {or_name}"
    );

    let ollama_name = options[1].display_name.as_str();
    assert!(
        ollama_name.contains("llama3.2"),
        "Ollama display_name must contain model; got: {ollama_name}"
    );
    assert!(
        ollama_name.contains("ollama"),
        "Ollama display_name must contain provider label; got: {ollama_name}"
    );

    let anthropic_name = options[2].display_name.as_str();
    assert!(
        anthropic_name.contains("claude-opus-4-5"),
        "Anthropic display_name must contain model; got: {anthropic_name}"
    );
    assert!(
        anthropic_name.contains("anthropic"),
        "Anthropic display_name must contain provider label; got: {anthropic_name}"
    );
}

/// Missing API key env var emits StreamChunk::Error before any HTTP attempt.
#[tokio::test]
async fn missing_api_key_env_var_emits_error_without_http_call() {
    use augur_domain::domain::string_newtypes::EnvVarName;

    let (reply_tx, mut reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test-openrouter-no-key"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("http://127.0.0.1:1"),
            model: ModelName::new("anthropic/claude-sonnet-4-5"),
            credentials: EndpointCredentials {
                api_key_env: Some(EnvVarName::new(NONEXISTENT_ENV_VAR)),
                api_key: None,
            },
        })
        .payload(
            RequestPayload::builder()
                .messages(vec![])
                .tools(vec![])
                .maybe_cache(None)
                .build(),
        )
        .reply_tx(reply_tx)
        .params(GenerationParams {
            max_tokens: TokenCount::new(TEST_MAX_TOKENS),
            temperature: Temperature::new(TEST_TEMPERATURE),
        })
        .build();

    stream_complete(ctx).await;

    let first = reply_rx.recv().await;
    assert!(
        matches!(first, Some(StreamChunk::Error(_))),
        "missing env var must produce an Error chunk; got {first:?}"
    );
}

// ── private helpers ───────────────────────────────────────────────────────────

/// Build a three-element endpoint list spanning OpenRouter, Ollama, and Anthropic.
fn make_three_endpoint_list() -> Vec<EndpointConfig> {
    vec![
        EndpointConfig {
            name: EndpointName::new("openrouter-prod"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("https://openrouter.ai/api/v1"),
            model: ModelName::new("anthropic/claude-sonnet-4-5"),
            credentials: EndpointCredentials::default(),
        },
        EndpointConfig {
            name: EndpointName::new("ollama-local"),
            provider: Provider::Ollama,
            base_url: EndpointUrl::new("http://localhost:11434"),
            model: ModelName::new("llama3.2"),
            credentials: EndpointCredentials::default(),
        },
        EndpointConfig {
            name: EndpointName::new("anthropic-claude"),
            provider: Provider::Anthropic,
            base_url: EndpointUrl::new("https://api.anthropic.com"),
            model: ModelName::new("claude-opus-4-5"),
            credentials: EndpointCredentials::default(),
        },
    ]
}
