use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, ModelName, OutputText, ToolDescription, ToolName,
};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_ollama::stream_complete;
use augur_provider_shared::request_context::{
    GenerationParams, RequestContext, RequestPayload, ToolDefinition,
};
use tokio::sync::mpsc;

fn make_ctx(base_url: &str) -> (RequestContext, mpsc::Receiver<StreamChunk>) {
    let (reply_tx, reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test"),
            provider: Provider::Ollama,
            base_url: EndpointUrl::new(base_url),
            model: ModelName::new("llama3.2"),
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
            max_tokens: TokenCount::new(4096),
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
        .with_body("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\ndata: [DONE]\n")
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::Token(OutputText::new("ok")))
    );
    match rx.recv().await {
        Some(StreamChunk::Usage(_)) => {}
        other => panic!("expected Usage chunk, got {other:?}"),
    }
    assert_eq!(rx.recv().await, Some(StreamChunk::Done));
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
            name: EndpointName::new("test"),
            provider: Provider::Ollama,
            base_url: EndpointUrl::new(server.url()),
            model: ModelName::new("llama3.2"),
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
