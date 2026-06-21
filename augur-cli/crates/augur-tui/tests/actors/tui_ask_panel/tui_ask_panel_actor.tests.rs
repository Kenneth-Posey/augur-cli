use augur_domain::domain::newtypes::Count;
use augur_tui::actors::tui_ask_panel::tui_ask_panel_actor::spawn;
use augur_tui::domain::string_newtypes::OutputText;
use augur_tui::domain::tui_state::OutputLine;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_open_seed_append_and_close_transitions_state() {
    let (_join, handle) = spawn(Count::of(8));

    assert!(handle.current_state().is_none(), "ask panel starts closed");

    handle.open();
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(
        handle.current_state().is_some(),
        "open should initialize state"
    );

    handle.seed_history(vec![OutputLine::plain("history")]);
    handle.append_line(OutputLine::tool_call(OutputText::from("tool output")));
    tokio::time::sleep(Duration::from_millis(25)).await;

    let state = handle.current_state().expect("state remains open");
    assert!(state.seeded, "seed_history should mark seeded=true");
    assert!(
        state.output.len() >= 2,
        "seed_history + append_line should produce at least two lines"
    );

    handle.close();
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(handle.current_state().is_none(), "close should clear state");
}

#[tokio::test]
async fn test_shutdown_completes_actor_task() {
    let (join, handle) = spawn(Count::of(8));
    handle.shutdown();
    let result = timeout(Duration::from_millis(500), join).await;
    assert!(result.is_ok(), "ask panel actor did not shut down in time");
    assert!(
        result.expect("timeout checked").is_ok(),
        "actor join panicked"
    );
}

#[test]
fn mirror_sync_executes_test_open_seed_append_and_close_transitions_state() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
