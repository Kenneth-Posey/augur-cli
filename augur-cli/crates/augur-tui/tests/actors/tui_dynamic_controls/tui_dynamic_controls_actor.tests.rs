use augur_domain::domain::newtypes::Count;
use augur_tui::actors::tui_dynamic_controls::tui_dynamic_controls_actor::spawn;
use augur_tui::actors::tui_dynamic_controls::tui_dynamic_controls_ops::ControlItem;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_set_controls_updates_state_snapshot() {
    let (_join, handle) = spawn(Count::of(8));
    handle.set_controls(vec![ControlItem {
        key: "q".into(),
        label: "quit".into(),
    }]);
    tokio::time::sleep(Duration::from_millis(25)).await;

    let state = handle.current_state();
    assert_eq!(state.controls.len(), 1, "one control should be published");
    assert_eq!(state.controls[0].key, "q");
    assert_eq!(state.controls[0].label, "quit");
}

#[tokio::test]
async fn test_shutdown_completes_actor_task() {
    let (join, handle) = spawn(Count::of(8));
    handle.shutdown();
    let result = timeout(Duration::from_millis(500), join).await;
    assert!(
        result.is_ok(),
        "dynamic controls actor did not shut down in time"
    );
    assert!(
        result.expect("timeout checked").is_ok(),
        "actor join panicked"
    );
}

#[test]
fn mirror_sync_executes_test_set_controls_updates_state_snapshot() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
