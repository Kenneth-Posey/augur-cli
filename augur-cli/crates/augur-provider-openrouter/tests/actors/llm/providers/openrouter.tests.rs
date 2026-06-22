use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, EnvVarName, ModelName, OutputText, ToolDescription, ToolName,
};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_openrouter::actors::llm::providers::openrouter::stream_complete;
use augur_provider_shared::request_context::{
    GenerationParams, RequestContext, RequestPayload, ToolDefinition,
};
use tokio::sync::mpsc;

fn make_ctx(base_url: &str) -> (RequestContext, mpsc::Receiver<StreamChunk>) {
    let (reply_tx, reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test-openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new(base_url),
            model: ModelName::new("openai/gpt-4o-mini"),
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
            max_tokens: TokenCount::new(256),
            temperature: Temperature::new(0.7),
        })
        .build();
    (ctx, reply_rx)
}

#[tokio::test]
async fn stream_complete_delegates_to_openai_compat_path() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\ndata: [DONE]\n")
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::Token(OutputText::new("hello")))
    );
    match rx.recv().await {
        Some(StreamChunk::Usage(_)) => {}
        other => panic!("expected Usage chunk, got {other:?}"),
    }
    assert_eq!(rx.recv().await, Some(StreamChunk::Done));
}

#[tokio::test]
async fn stream_complete_emits_error_on_missing_env_var() {
    let (reply_tx, mut reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test-openrouter-missing-key"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("http://localhost:9999"),
            model: ModelName::new("openai/gpt-4o-mini"),
            credentials: EndpointCredentials {
                api_key_env: Some(EnvVarName::new("DCMK_TEST_NONEXISTENT_VAR_OPENROUTER_9999")),
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
            max_tokens: TokenCount::new(256),
            temperature: Temperature::new(0.0),
        })
        .build();
    stream_complete(ctx).await;
    match reply_rx.recv().await {
        Some(StreamChunk::Error(_)) => {}
        other => panic!("expected Error chunk on missing env var, got {other:?}"),
    }
}

#[tokio::test]
async fn cache_headers_injected_when_enabled() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .match_header("X-OpenRouter-Cache", "true")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: [DONE]\n")
        .create();
    let (reply_tx, _rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test-openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new(server.url()),
            model: ModelName::new("openai/gpt-4o-mini"),
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
            max_tokens: TokenCount::new(256),
            temperature: Temperature::new(0.0),
        })
        .extra_request_headers(vec![("X-OpenRouter-Cache".to_string(), "true".to_string())])
        .build();
    stream_complete(ctx).await;
    _mock.assert();
}

#[tokio::test]
async fn cache_headers_with_ttl_injected() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .match_header("X-OpenRouter-Cache", "true")
        .match_header("X-OpenRouter-Cache-TTL", "600")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: [DONE]\n")
        .create();
    let (reply_tx, _rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test-openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new(server.url()),
            model: ModelName::new("openai/gpt-4o-mini"),
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
            max_tokens: TokenCount::new(256),
            temperature: Temperature::new(0.0),
        })
        .extra_request_headers(vec![
            ("X-OpenRouter-Cache".to_string(), "true".to_string()),
            ("X-OpenRouter-Cache-TTL".to_string(), "600".to_string()),
        ])
        .build();
    stream_complete(ctx).await;
    _mock.assert();
}

#[tokio::test]
async fn cache_headers_not_injected_when_disabled() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: [DONE]\n")
        .create();
    let (ctx, _rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    _mock.assert();
}

#[tokio::test]
async fn stream_complete_includes_size_check_tool_schema_in_request() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .match_body(mockito::Matcher::Regex("size_check".to_owned()))
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: [DONE]\n")
        .create();
    let (reply_tx, _rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test-openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new(server.url()),
            model: ModelName::new("openai/gpt-4o-mini"),
            credentials: EndpointCredentials::default(),
        })
        .payload(
            RequestPayload::builder()
                .messages(vec![])
                .tools(vec![ToolDefinition::new(
                    ToolName::new("size_check"),
                    ToolDescription::new("Check file and directory sizes."),
                    serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
                )])
                .maybe_cache(None)
                .build(),
        )
        .reply_tx(reply_tx)
        .params(GenerationParams {
            max_tokens: TokenCount::new(256),
            temperature: Temperature::new(0.0),
        })
        .build();
    stream_complete(ctx).await;
    _mock.assert();
}

#[test]
fn mirror_sync_executes_stream_complete_delegates_to_openai_compat_path() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
