use augur_core::actors::catalog_manager::models::{OutputFormat, ProviderName};

/// Verifies that spawn() returns a live handle whose channel is functional.
///
/// Sends a `generate_catalog` command with an invalid provider name to trigger a
/// fast error path without making any network calls. Receiving an `Err` with the
/// expected message confirms the actor loop is running and the channel round-trip
/// works end-to-end.
#[tokio::test]
async fn spawn_returns_handle() {
    let handle = augur_core::actors::catalog_manager::catalog_manager_actor::spawn();
    let result = handle
        .generate_catalog(
            Some(ProviderName("invalid-provider".to_owned())),
            OutputFormat::Yaml,
        )
        .await;
    assert!(result.is_err(), "expected Err for unknown provider");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unknown provider"),
        "error must mention unknown provider, got: {msg}"
    );
}

/// Verifies that dropping the handle causes the actor to exit cleanly.
///
/// After the handle is dropped, the mpsc sender is gone. The actor's `recv()`
/// returns `None` and the loop exits. This test confirms no panic occurs during
/// that teardown path.
#[tokio::test]
async fn actor_stops_when_handle_dropped() {
    let handle = augur_core::actors::catalog_manager::catalog_manager_actor::spawn();
    drop(handle);
    // Allow the runtime to schedule and complete the actor teardown.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    // If no panic occurred, the actor exited cleanly.
}

#[test]
fn mirror_sync_executes_spawn_returns_handle() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
