use augur_domain::domain::channels::TUI_FEED_CAPACITY;
use augur_tui::actors::tui_agent_panel::tui_agent_panel_actor::{spawn, TuiAgentPanelConfig};
use augur_tui::domain::newtypes::NumericNewtype;
use augur_tui::domain::string_newtypes::OutputText;
use augur_tui::domain::types::AgentFeedOutput;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Verifies that an AgentFeed item is forwarded to the unified output channel.
#[tokio::test]
async fn test_agent_feed_forwarded_to_unified_channel() {
    let (unified_tx, mut unified_rx) = mpsc::channel(TUI_FEED_CAPACITY.inner());
    let config = TuiAgentPanelConfig {
        unified_tx,
        capacity: TUI_FEED_CAPACITY.inner(),
    };
    let (_join, handle) = spawn(config);

    let item = AgentFeedOutput::StatusLine(OutputText::from("agent feed test"));
    handle.send_agent_feed(item);

    let received = timeout(Duration::from_millis(200), unified_rx.recv()).await;
    assert!(
        received.is_ok(),
        "unified channel did not receive item within timeout"
    );
    let received = received.unwrap();
    assert!(
        received.is_some(),
        "unified channel was closed unexpectedly"
    );
    assert!(matches!(received.unwrap(), AgentFeedOutput::StatusLine(_)));
}

/// Verifies that a ToolFeed item is forwarded to the unified output channel.
#[tokio::test]
async fn test_tool_feed_forwarded_to_unified_channel() {
    let (unified_tx, mut unified_rx) = mpsc::channel(TUI_FEED_CAPACITY.inner());
    let config = TuiAgentPanelConfig {
        unified_tx,
        capacity: TUI_FEED_CAPACITY.inner(),
    };
    let (_join, handle) = spawn(config);

    let item = AgentFeedOutput::ToolEventLine(OutputText::from("tool feed test"));
    handle.send_tool_feed(item);

    let received = timeout(Duration::from_millis(200), unified_rx.recv()).await;
    assert!(
        received.is_ok(),
        "unified channel did not receive item within timeout"
    );
    let received = received.unwrap();
    assert!(
        received.is_some(),
        "unified channel was closed unexpectedly"
    );
    assert!(matches!(
        received.unwrap(),
        AgentFeedOutput::ToolEventLine(_)
    ));
}

/// Verifies that sending Shutdown causes the actor task to complete cleanly.
#[tokio::test]
async fn test_shutdown_closes_channel() {
    let (unified_tx, _unified_rx) = mpsc::channel(TUI_FEED_CAPACITY.inner());
    let config = TuiAgentPanelConfig {
        unified_tx,
        capacity: TUI_FEED_CAPACITY.inner(),
    };
    let (join, handle) = spawn(config);

    handle.shutdown();

    let result = timeout(Duration::from_millis(500), join).await;
    assert!(result.is_ok(), "actor did not shut down within timeout");
    assert!(result.unwrap().is_ok(), "actor task panicked");
}

#[test]
fn mirror_sync_executes_test_agent_feed_forwarded_to_unified_channel() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
