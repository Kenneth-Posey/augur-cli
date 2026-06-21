use augur_core::actors::user_message_consumer::user_message_consumer_actor::{
    spawn, UserMessageOutputChannels,
};
use std::fs;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

#[test]
fn handle_source_exposes_process_and_shutdown_methods() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/user_message_consumer/handle.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("handle source must be readable");
    assert!(source.contains("fn process_input(&self"));
    assert!(source.contains("pub fn shutdown(&self)"));
}

#[tokio::test]
async fn handle_shutdown_stops_actor() {
    let (raw_tx, _raw_rx) = mpsc::channel(8);
    let (parsed_tx, _parsed_rx) = mpsc::channel(8);
    let outputs = UserMessageOutputChannels { raw_tx, parsed_tx };
    let (join, handle) = spawn(outputs);

    handle.shutdown();
    let result = timeout(Duration::from_secs(1), join).await;
    assert!(result.is_ok(), "actor should stop after handle shutdown");
}
