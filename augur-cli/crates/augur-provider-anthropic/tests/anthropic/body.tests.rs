use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, ModelName, ToolDescription, ToolName,
};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_anthropic::stream_complete;
use augur_provider_shared::request_context::{
    GenerationParams, RequestContext, RequestPayload, ToolDefinition,
};
use tokio::sync::mpsc;

fn make_ctx_with_tools(
    base_url: &str,
    tools: Vec<ToolDefinition>,
) -> (RequestContext, mpsc::Receiver<StreamChunk>) {
    let (reply_tx, reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test"),
            provider: Provider::Anthropic,
            base_url: EndpointUrl::new(base_url),
            model: ModelName::new("claude-opus-4-6"),
            credentials: EndpointCredentials::default(),
        })
        .payload(
            RequestPayload::builder()
                .messages(vec![])
                .tools(tools)
                .maybe_cache(None)
                .build(),
        )
        .reply_tx(reply_tx)
        .params(GenerationParams {
            max_tokens: TokenCount::new(256),
            temperature: Temperature::new(0.0),
        })
        .build();
    (ctx, reply_rx)
}

#[tokio::test]
async fn stream_complete_includes_tool_schema_in_anthropic_request_body() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/messages")
        .match_body(mockito::Matcher::Regex("size_check".to_owned()))
        .match_body(mockito::Matcher::Regex("input_schema".to_owned()))
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("event: message_stop\ndata: {}\n\n")
        .create();
    let (ctx, _rx) = make_ctx_with_tools(
        &server.url(),
        vec![ToolDefinition::new(
            ToolName::new("size_check"),
            ToolDescription::new("Check file and directory sizes."),
            serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        )],
    );
    stream_complete(ctx).await;
    _mock.assert();
}
