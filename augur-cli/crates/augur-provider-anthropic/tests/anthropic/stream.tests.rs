use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::{EndpointName, EndpointUrl, ModelName, OutputText};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_anthropic::stream_complete;
use augur_provider_shared::request_context::{GenerationParams, RequestContext, RequestPayload};
use tokio::sync::mpsc;

fn make_ctx(base_url: &str) -> (RequestContext, mpsc::Receiver<StreamChunk>) {
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
async fn stream_complete_mock_sends_tokens_then_done() {
    let mut server = mockito::Server::new_async().await;
    let body = concat!(
        "event: content_block_delta\n",
        "data: {\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
        "event: message_stop\n",
        "data: {}\n\n",
    );
    let _mock = server
        .mock("POST", "/messages")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::Token(OutputText::new("hi")))
    );
    match rx.recv().await {
        Some(StreamChunk::Usage(_)) => {}
        other => panic!("expected Usage chunk, got {other:?}"),
    }
    assert_eq!(rx.recv().await, Some(StreamChunk::Done));
}

#[tokio::test]
async fn stream_complete_mock_http_error_sends_error_chunk() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/messages")
        .with_status(401)
        .with_body("{\"error\":\"unauthorized\"}")
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    match rx.recv().await {
        Some(StreamChunk::Error(msg)) => {
            assert!(msg.contains("401"), "expected 401 in '{msg}'");
            assert!(
                msg.contains("unauthorized"),
                "expected body text in '{msg}'"
            );
        }
        other => panic!("expected Error chunk, got {other:?}"),
    }
}

#[tokio::test]
async fn model_falls_back_to_endpoint_when_stream_omits_it() {
    let mut server = mockito::Server::new_async().await;
    let body = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
        "event: message_stop\n",
        "data: {}\n\n",
    );
    let _mock = server
        .mock("POST", "/messages")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::Token(OutputText::new("hi")))
    );
    match rx.recv().await {
        Some(StreamChunk::Usage(u)) => {
            assert_eq!(u.model.as_str(), "claude-opus-4-6");
        }
        other => panic!("expected Usage chunk, got {other:?}"),
    }
    assert_eq!(rx.recv().await, Some(StreamChunk::Done));
}
