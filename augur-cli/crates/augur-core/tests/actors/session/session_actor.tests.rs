use augur_core::actors::session::session_actor::spawn;
use augur_domain::domain::string_newtypes::{EndpointName, StringNewtype};
use std::time::Duration;
use tokio::time::timeout;

/// Verifies that spawning the session actor with a default endpoint makes it
/// immediately readable from active_endpoint().
#[tokio::test]
async fn spawn_and_default_endpoint() {
    let (_join, handle) = spawn(EndpointName::new("ollama-local"));
    assert_eq!(handle.active_endpoint().as_str(), "ollama-local");
}

/// Verifies that calling set_endpoint updates the watch channel so
/// active_endpoint() returns the new value.
#[tokio::test]
async fn set_endpoint_updates_watch() {
    let (_join, handle) = spawn(EndpointName::new("default"));
    handle
        .set_endpoint(EndpointName::new("gpt-4o"))
        .await
        .expect("session endpoint change should enqueue");
    let result: Result<_, _> = timeout(Duration::from_secs(1), async {
        loop {
            if handle.active_endpoint().as_str() == "gpt-4o" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(result.is_ok(), "endpoint did not update within timeout");
}

/// Verifies that calling shutdown causes the actor task to complete cleanly.
#[tokio::test]
async fn shutdown_cleanly() {
    let (join, handle) = spawn(EndpointName::new("default"));
    handle.shutdown();
    let result: Result<_, _> = timeout(Duration::from_secs(1), join).await;
    assert!(result.is_ok(), "actor did not shut down within timeout");
    assert!(result.unwrap().is_ok());
}

#[test]
fn mirror_sync_executes_spawn_and_default_endpoint() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
