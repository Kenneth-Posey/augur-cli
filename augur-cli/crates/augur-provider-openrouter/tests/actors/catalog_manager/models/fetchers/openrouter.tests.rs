use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{EndpointName, EndpointUrl, EnvVarName, ModelName};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_openrouter::actors::llm::providers::openrouter::stream_complete;
use augur_provider_shared::request_context::{GenerationParams, RequestContext, RequestPayload};
use tokio::sync::mpsc;

#[tokio::test]
async fn fetcher_openrouter_stream_complete_reports_missing_env_var_error() {
    let (reply_tx, mut reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("catalog-fetch-openrouter"),
            provider: Provider::OpenRouter,
            base_url: EndpointUrl::new("http://localhost:1"),
            model: ModelName::new("openai/gpt-4o-mini"),
            credentials: EndpointCredentials {
                api_key_env: Some(EnvVarName::new("DCMK_MISSING_OPENROUTER_KEY_FOR_TEST")),
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
            max_tokens: TokenCount::new(64),
            temperature: Temperature::new(0.0),
        })
        .build();

    stream_complete(ctx).await;

    assert!(
        matches!(reply_rx.recv().await, Some(StreamChunk::Error(_))),
        "missing env var path must surface deterministic error chunk",
    );
}
