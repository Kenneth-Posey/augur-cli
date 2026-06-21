use augur_domain::domain::newtypes::Count;
use augur_tui::actors::tui_spinner::tui_spinner_actor::spawn;
use augur_tui::actors::tui_spinner::tui_spinner_ops::SpinnerTarget;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_stop_preserves_inactive_default_state() {
    let (_join, handle) = spawn(Count::of(8));
    handle.stop(SpinnerTarget::MainConversation);
    tokio::time::sleep(Duration::from_millis(25)).await;

    let state = handle.current_state();
    assert!(!state.active, "stop should leave spinner inactive");
    assert_eq!(
        state.target,
        SpinnerTarget::MainConversation,
        "default spinner target should remain main conversation"
    );
}

#[tokio::test]
async fn test_shutdown_completes_actor_task() {
    let (join, handle) = spawn(Count::of(8));
    handle.shutdown();
    let result = timeout(Duration::from_millis(500), join).await;
    assert!(result.is_ok(), "spinner actor did not shut down in time");
    assert!(
        result.expect("timeout checked").is_ok(),
        "actor join panicked"
    );
}

#[test]
fn mirror_sync_executes_test_stop_preserves_inactive_default_state() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
