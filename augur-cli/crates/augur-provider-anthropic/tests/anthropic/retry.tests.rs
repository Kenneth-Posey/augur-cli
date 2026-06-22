use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{Temperature, TokenCount, WaitSecs};
use augur_domain::domain::string_newtypes::{EndpointName, EndpointUrl, ModelName, OutputText};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_anthropic::stream_complete;
use augur_provider_shared::MAX_RETRY_ATTEMPTS;
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
async fn stream_complete_rate_limit_retries_and_succeeds() {
    let mut server = mockito::Server::new_async().await;
    let _mock_429 = server
        .mock("POST", "/messages")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body("{\"error\":\"rate limited\"}")
        .expect(1)
        .create();
    let body = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":0,\"cache_read_input_tokens\":0}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let _mock_ok = server
        .mock("POST", "/messages")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(body)
        .expect(1)
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::RateLimitRetry(WaitSecs::new(0)))
    );
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::Token(OutputText::new("ok")))
    );
    match rx.recv().await {
        Some(StreamChunk::Usage(_)) => {}
        other => panic!("expected Usage, got {other:?}"),
    }
    assert_eq!(rx.recv().await, Some(StreamChunk::Done));
}

#[tokio::test]
async fn stream_complete_rate_limit_exhausted_sends_error() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/messages")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body("{\"error\":\"rate limited\"}")
        .expect(MAX_RETRY_ATTEMPTS)
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_complete(ctx).await;
    for _ in 0..MAX_RETRY_ATTEMPTS {
        assert_eq!(
            rx.recv().await,
            Some(StreamChunk::RateLimitRetry(WaitSecs::new(0)))
        );
    }
    match rx.recv().await {
        Some(StreamChunk::Error(msg)) => {
            assert!(msg.contains("exhausted"), "expected 'exhausted' in '{msg}'");
        }
        other => panic!("expected Error after exhausted retries, got {other:?}"),
    }
}
