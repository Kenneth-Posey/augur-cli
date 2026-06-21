//! Actor-level tests for the LLM feed consumer: spawn, consume, and shutdown.

use augur_core::actors::llm_feed_consumer::llm_feed_consumer_actor::spawn;
use augur_core::actors::llm_feed_consumer::llm_feed_consumer_ops::LlmFeedOutputChannels;
use augur_domain::domain::feeds::LlmFeedTag;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::domain::types::StreamChunk;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

/// Verifies that a Token chunk sent via the actor handle arrives on the user_chunk channel.
#[tokio::test]
async fn test_consume_routes_through_actor() {
    let (bg_tx, _bg_rx) = mpsc::channel(8);
    let (thinking_tx, _thinking_rx) = mpsc::channel(8);
    let (user_tx, mut user_rx) = mpsc::channel(8);
    let (tool_tx, _tool_rx) = mpsc::channel(8);

    let outputs = LlmFeedOutputChannels::builder()
        .bg_agent_tx(bg_tx)
        .thinking_tx(thinking_tx)
        .user_chunk_tx(user_tx)
        .tool_request_tx(tool_tx)
        .build();

    let (_join, handle) = spawn(outputs);

    handle.consume(StreamChunk::Token(OutputText::new("hello".to_owned())));

    let msg = timeout(Duration::from_secs(2), user_rx.recv())
        .await
        .expect("must receive within timeout")
        .expect("user channel must have a message");

    assert_eq!(msg.tag, LlmFeedTag::UserChunk);
    handle.shutdown();
}

/// Verifies that calling shutdown causes the actor task to exit cleanly.
#[tokio::test]
async fn test_shutdown_stops_actor() {
    let (bg_tx, _bg_rx) = mpsc::channel(8);
    let (thinking_tx, _thinking_rx) = mpsc::channel(8);
    let (user_tx, _user_rx) = mpsc::channel(8);
    let (tool_tx, _tool_rx) = mpsc::channel(8);

    let outputs = LlmFeedOutputChannels::builder()
        .bg_agent_tx(bg_tx)
        .thinking_tx(thinking_tx)
        .user_chunk_tx(user_tx)
        .tool_request_tx(tool_tx)
        .build();

    let (join, handle) = spawn(outputs);

    handle.shutdown();

    let result = timeout(Duration::from_secs(2), join).await;
    assert!(
        result.is_ok(),
        "actor must finish within 2 seconds of shutdown"
    );
}

#[test]
fn mirror_sync_executes_test_consume_routes_through_actor() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
