use augur_cli::wiring::{
    actor_runtime, forward_reply_to_broadcast, spawn_root_deterministic_orchestrator_runtime,
};
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount, WaitSecs};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolCallId, ToolName};
use augur_domain::domain::types::{AgentOutput, LlmTokenCounts, LlmUsage, StreamChunk};

#[test]
fn mirrored_surface_smoke_app_runtime() {
    let function_name = core::any::type_name_of_val(&forward_reply_to_broadcast);
    assert!(function_name.contains("forward_reply_to_broadcast"));
    let function_name = core::any::type_name_of_val(&actor_runtime::<()>);
    assert!(function_name.contains("actor_runtime"));
    let function_name = core::any::type_name_of_val(&spawn_root_deterministic_orchestrator_runtime);
    assert!(function_name.contains("spawn_root_deterministic_orchestrator_runtime"));
}

#[tokio::test]
async fn actor_runtime_wraps_join_and_handle_pair() {
    let join = tokio::spawn(async {});
    let runtime = actor_runtime((join, 7_u8));
    assert_eq!(runtime.handle, 7_u8);
}

#[tokio::test]
async fn spawn_root_deterministic_orchestrator_runtime_produces_live_handle() {
    let (feed_tx, _feed_rx) =
        tokio::sync::mpsc::channel::<augur_domain::domain::types::FeedEntry>(8);
    let runtime = spawn_root_deterministic_orchestrator_runtime(feed_tx);
    let _events = runtime.handle.subscribe();
    runtime.handle.shutdown();
    let _ = runtime.join.await;
}

#[tokio::test]
async fn forward_reply_to_broadcast_forwards_token_then_stops_on_done() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let (out_tx, mut out_rx) = tokio::sync::broadcast::channel(8);
    tx.send(StreamChunk::Token(OutputText::new("hello")))
        .await
        .expect("send token");
    tx.send(StreamChunk::Done).await.expect("send done");
    tx.send(StreamChunk::Token(OutputText::new("ignored")))
        .await
        .expect("send ignored");
    drop(tx);
    forward_reply_to_broadcast(rx, out_tx).await;
    assert!(matches!(
        out_rx.try_recv(),
        Ok(AgentOutput::Token(text)) if text.as_str() == "hello"
    ));
    assert!(
        out_rx.try_recv().is_err(),
        "processing should stop at Done marker"
    );
}

#[tokio::test]
async fn forward_reply_to_broadcast_forwards_error_and_stops() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let (out_tx, mut out_rx) = tokio::sync::broadcast::channel(8);
    tx.send(StreamChunk::Error(OutputText::new("boom")))
        .await
        .expect("send error");
    tx.send(StreamChunk::Token(OutputText::new("ignored")))
        .await
        .expect("send ignored");
    drop(tx);
    forward_reply_to_broadcast(rx, out_tx).await;
    assert!(matches!(
        out_rx.try_recv(),
        Ok(AgentOutput::Error(text)) if text.as_str() == "boom"
    ));
    assert!(
        out_rx.try_recv().is_err(),
        "processing should stop after error"
    );
}

#[tokio::test]
async fn forward_reply_to_broadcast_emits_rate_limit_notice_and_backoff() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let (out_tx, mut out_rx) = tokio::sync::broadcast::channel(8);
    tx.send(StreamChunk::RateLimitRetry(WaitSecs::new(4)))
        .await
        .expect("send backoff");
    drop(tx);
    forward_reply_to_broadcast(rx, out_tx).await;
    assert!(matches!(
        out_rx.try_recv(),
        Ok(AgentOutput::Token(text)) if text.as_str().contains("rate limit")
    ));
    assert!(matches!(
        out_rx.try_recv(),
        Ok(AgentOutput::BackoffStarted(secs)) if secs.inner() == 4
    ));
}

#[tokio::test]
async fn forward_reply_to_broadcast_ignores_toolcall_and_usage_chunks() {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let (out_tx, mut out_rx) = tokio::sync::broadcast::channel(8);
    tx.send(StreamChunk::ToolCall {
        id: ToolCallId::new("tool-1"),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({}),
    })
    .await
    .expect("send tool call");
    tx.send(StreamChunk::Usage(LlmUsage {
        model: OutputText::new("model-x"),
        token_counts: LlmTokenCounts::builder()
            .tokens_in(TokenCount::new(1))
            .tokens_out(TokenCount::new(2))
            .tokens_cached(TokenCount::new(0))
            .build(),
        temperature: Temperature::new(0.0),
    }))
    .await
    .expect("send usage");
    drop(tx);
    forward_reply_to_broadcast(rx, out_tx).await;
    assert!(
        out_rx.try_recv().is_err(),
        "tool-call/usage should not emit agent output"
    );
}
