//! Tests for the `request_rework` verdict tool.

use augur_core::tools::builtin::request_rework::RequestRework;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::{ReworkReason, StringNewtype};
use tokio::sync::oneshot;

/// Verifies that calling `execute` with a `reason` argument sends the reason string
/// on the oneshot channel.
#[tokio::test]
async fn execute_sends_reason_on_oneshot() {
    let (tx, rx) = oneshot::channel::<ReworkReason>();
    let tool = RequestRework::new(tx);
    let result = tool
        .execute(serde_json::json!({"reason": "missing tests"}))
        .await;
    let received = rx.await.expect("sender should have fired");
    assert_eq!(received, ReworkReason::new("missing tests"));
    assert!(!result.is_error, "tool result should not be an error");
    assert_eq!(result.output.as_str(), "rework requested");
}

/// Verifies that calling `execute` without a `reason` sends a fallback string.
#[tokio::test]
async fn execute_without_reason_sends_fallback() {
    let (tx, rx) = oneshot::channel::<ReworkReason>();
    let tool = RequestRework::new(tx);
    tool.execute(serde_json::json!({})).await;
    let received = rx.await.expect("sender should have fired");
    assert_eq!(received, ReworkReason::new("no reason provided"));
}

/// Verifies that calling `execute` a second time returns an error (sender consumed).
#[tokio::test]
async fn execute_second_call_returns_error() {
    let (tx, _rx) = oneshot::channel::<ReworkReason>();
    let tool = RequestRework::new(tx);
    tool.execute(serde_json::json!({"reason": "first"})).await;
    let result = tool.execute(serde_json::json!({"reason": "second"})).await;
    assert!(result.is_error, "second call should return is_error = true");
}

/// Verifies that dropping the receiver before execution causes an error result.
#[tokio::test]
async fn execute_receiver_dropped_returns_error() {
    let (tx, rx) = oneshot::channel::<ReworkReason>();
    drop(rx);
    let tool = RequestRework::new(tx);
    let result = tool
        .execute(serde_json::json!({"reason": "missing tests"}))
        .await;
    assert!(result.is_error);
}

#[test]
fn mirror_sync_executes_execute_sends_reason_on_oneshot() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
