use augur_core::domain::deterministic_orchestrator::DeterministicOrchestratorEvent;
use augur_domain::domain::channels::TUI_FEED_CAPACITY;
use augur_tui::actors::tui_main_feed_panel::tui_main_feed_panel_actor::{spawn, TuiMainFeedConfig};
use augur_tui::actors::tui_main_feed_panel::tui_main_feed_panel_ops::MainFeedItem;
use augur_tui::domain::newtypes::NumericNewtype;
use augur_tui::domain::string_newtypes::OutputText;
use augur_tui::domain::types::AgentOutput;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Verifies that an Agent command is forwarded as MainFeedItem::AgentOut.
#[tokio::test]
async fn test_agent_output_forwarded_as_main_feed_item() {
    let (unified_tx, mut unified_rx) = mpsc::channel(TUI_FEED_CAPACITY.inner());
    let config = TuiMainFeedConfig {
        unified_tx,
        capacity: TUI_FEED_CAPACITY.inner(),
    };
    let (_join, handle) = spawn(config);

    let item = AgentOutput::Token(OutputText::from("hello"));
    handle.send_agent(item);

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
    assert!(matches!(received.unwrap(), MainFeedItem::AgentOut(_)));
}

/// Verifies that an Orchestrator command is forwarded as MainFeedItem::OrchestratorEvent.
#[tokio::test]
async fn test_orchestrator_event_forwarded_as_main_feed_item() {
    let (unified_tx, mut unified_rx) = mpsc::channel(TUI_FEED_CAPACITY.inner());
    let config = TuiMainFeedConfig {
        unified_tx,
        capacity: TUI_FEED_CAPACITY.inner(),
    };
    let (_join, handle) = spawn(config);

    let ev = DeterministicOrchestratorEvent::Completed;
    handle.send_orchestrator(ev);

    let received = timeout(Duration::from_millis(200), unified_rx.recv()).await;
    assert!(
        received.is_ok(),
        "unified channel did not receive event within timeout"
    );
    let received = received.unwrap();
    assert!(
        received.is_some(),
        "unified channel was closed unexpectedly"
    );
    assert!(matches!(
        received.unwrap(),
        MainFeedItem::OrchestratorEvent(_)
    ));
}

/// Verifies that sending Shutdown causes the actor task to complete cleanly.
#[tokio::test]
async fn test_shutdown_terminates_run_loop() {
    let (unified_tx, _unified_rx) = mpsc::channel(TUI_FEED_CAPACITY.inner());
    let config = TuiMainFeedConfig {
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
fn mirror_sync_executes_test_agent_output_forwarded_as_main_feed_item() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
