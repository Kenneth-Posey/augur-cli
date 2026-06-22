use augur_core::actors::session::session_actor::spawn;
use augur_domain::domain::string_newtypes::{EndpointName, StringNewtype};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn active_endpoint_reflects_default_and_updates() {
    let (_join, handle) = spawn(EndpointName::new("default-endpoint"));
    assert_eq!(handle.active_endpoint().as_str(), "default-endpoint");

    handle
        .set_endpoint(EndpointName::new("updated-endpoint"))
        .await
        .expect("set_endpoint should enqueue");

    let result: Result<_, _> = timeout(Duration::from_secs(1), async {
        loop {
            if handle.active_endpoint().as_str() == "updated-endpoint" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(result.is_ok(), "endpoint update must become visible");
}

#[tokio::test]
async fn shutdown_stops_session_actor() {
    let (join, handle) = spawn(EndpointName::new("default"));
    handle.shutdown();
    let result: Result<_, _> = timeout(Duration::from_secs(1), join).await;
    assert!(result.is_ok(), "session actor must stop after shutdown");
}
