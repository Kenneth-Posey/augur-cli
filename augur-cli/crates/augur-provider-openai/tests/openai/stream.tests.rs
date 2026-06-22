use augur_domain::config::types::{EndpointConfig, EndpointCredentials, Provider};
use augur_domain::domain::channels::STREAM_CHUNK_CAPACITY;
use augur_domain::domain::newtypes::{Temperature, TokenCount, WaitSecs};
use augur_domain::domain::string_newtypes::{
    EndpointName, EndpointUrl, ModelName, OutputText, ToolDescription, ToolName,
};
use augur_domain::domain::types::StreamChunk;
use augur_domain::{NumericNewtype, StringNewtype};
use augur_provider_openai::stream_openai_compat;
use augur_provider_shared::request_context::{
    GenerationParams, RequestContext, RequestPayload, ToolDefinition,
};
use augur_provider_shared::MAX_RETRY_ATTEMPTS;
use tokio::sync::mpsc;

fn make_ctx(base_url: &str) -> (RequestContext, mpsc::Receiver<StreamChunk>) {
    let (reply_tx, reply_rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test"),
            provider: Provider::OpenAi,
            base_url: EndpointUrl::new(base_url),
            model: ModelName::new("gpt-4"),
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
async fn stream_complete_mock_sends_two_tokens_then_done() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\
             data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\
             data: [DONE]\n",
        )
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_openai_compat(ctx, None).await;
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::Token(OutputText::new("hello")))
    );
    assert_eq!(
        rx.recv().await,
        Some(StreamChunk::Token(OutputText::new(" world")))
    );
    match rx.recv().await {
        Some(StreamChunk::Usage(_)) => {}
        other => panic!("expected Usage before Done, got {other:?}"),
    }
    assert_eq!(rx.recv().await, Some(StreamChunk::Done));
}

#[tokio::test]
async fn stream_complete_mock_http_error_sends_error_chunk() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .with_status(500)
        .with_body("{\"error\":\"internal server error\"}")
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_openai_compat(ctx, None).await;
    match rx.recv().await {
        Some(StreamChunk::Error(msg)) => {
            assert!(msg.contains("500"), "expected 500 in '{msg}'");
            assert!(
                msg.contains("internal server error"),
                "expected body text in '{msg}'"
            );
        }
        other => panic!("expected Error chunk, got {other:?}"),
    }
}

#[tokio::test]
async fn stream_complete_rate_limit_retries_and_succeeds() {
    let mut server = mockito::Server::new_async().await;
    let _mock_429 = server
        .mock("POST", "/chat/completions")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body("{\"error\":\"rate limited\"}")
        .expect(1)
        .create();
    let _mock_ok = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\ndata: [DONE]\n")
        .expect(1)
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_openai_compat(ctx, None).await;
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
        .mock("POST", "/chat/completions")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body("{\"error\":\"rate limited\"}")
        .expect(MAX_RETRY_ATTEMPTS)
        .create();
    let (ctx, mut rx) = make_ctx(&server.url());
    stream_openai_compat(ctx, None).await;
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

#[tokio::test]
async fn extra_headers_are_sent_in_request() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("POST", "/chat/completions")
        .match_header("X-Custom-Header", "test-value")
        .match_header("X-Another-Header", "another-value")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body("data: [DONE]\n")
        .create();
    let (reply_tx, _rx) = mpsc::channel(*STREAM_CHUNK_CAPACITY);
    let ctx = RequestContext::builder()
        .endpoint(EndpointConfig {
            name: EndpointName::new("test"),
            provider: Provider::OpenAi,
            base_url: EndpointUrl::new(server.url()),
            model: ModelName::new("gpt-4"),
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
        .extra_request_headers(vec![
            ("X-Custom-Header".to_string(), "test-value".to_string()),
            ("X-Another-Header".to_string(), "another-value".to_string()),
        ])
        .build();
    stream_openai_compat(ctx, None).await;
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
            name: EndpointName::new("test"),
            provider: Provider::OpenAi,
            base_url: EndpointUrl::new(server.url()),
            model: ModelName::new("gpt-4"),
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
    stream_openai_compat(ctx, None).await;
    _mock.assert();
}
