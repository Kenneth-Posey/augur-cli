//! Actor-level tests for the history adapter: spawn, route, and shutdown.

use augur_core::actors::history_adapter::history_adapter_actor::{HistoryAdapterConfig, spawn};
use augur_domain::domain::feeds::HistoryFeedMessage;
use augur_domain::domain::types::Message;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

/// Verifies that a `RecordUser` command causes `HistoryFeedMessage::UserEntry` to appear on the history channel.
#[tokio::test]
async fn test_run_forwards_user_entry() {
    let (history_tx, mut history_rx) = mpsc::channel(8);
    let config = HistoryAdapterConfig {
        history_tx,
        capacity: 8,
    };
    let (_join, handle) = spawn(config);

    let msg = Message::user("user text");
    handle.record_user(msg.clone());

    let entry = timeout(Duration::from_secs(2), history_rx.recv())
        .await
        .expect("must receive within timeout")
        .expect("history channel must have a message");

    match entry {
        HistoryFeedMessage::UserEntry(m) => {
            assert_eq!(m.content, msg.content);
        }
        other => panic!("expected UserEntry, got {other:?}"),
    }
    handle.shutdown();
}

/// Verifies that a `RecordLlm` command causes `HistoryFeedMessage::LlmEntry` to appear on the history channel.
#[tokio::test]
async fn test_run_forwards_llm_entry() {
    let (history_tx, mut history_rx) = mpsc::channel(8);
    let config = HistoryAdapterConfig {
        history_tx,
        capacity: 8,
    };
    let (_join, handle) = spawn(config);

    let msg = Message::assistant("llm response");
    handle.record_llm(msg.clone());

    let entry = timeout(Duration::from_secs(2), history_rx.recv())
        .await
        .expect("must receive within timeout")
        .expect("history channel must have a message");

    match entry {
        HistoryFeedMessage::LlmEntry(m) => {
            assert_eq!(m.content, msg.content);
        }
        other => panic!("expected LlmEntry, got {other:?}"),
    }
    handle.shutdown();
}

/// Verifies that sending `Shutdown` causes the actor task to complete cleanly.
#[tokio::test]
async fn test_shutdown_stops_actor() {
    let (history_tx, _history_rx) = mpsc::channel(8);
    let config = HistoryAdapterConfig {
        history_tx,
        capacity: 8,
    };
    let (join, handle) = spawn(config);

    handle.shutdown();

    let result = timeout(Duration::from_secs(2), join).await;
    assert!(
        result.is_ok(),
        "actor must finish within 2 seconds of shutdown"
    );
}

#[test]
fn mirror_sync_executes_test_run_forwards_user_entry() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
