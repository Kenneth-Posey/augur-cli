use augur_core::actors::user_message_consumer::user_message_consumer_actor::{
    spawn, UserMessageOutputChannels,
};
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn actor_spawn_and_shutdown_are_clean() {
    let (raw_tx, _raw_rx) = mpsc::channel(8);
    let (parsed_tx, _parsed_rx) = mpsc::channel(8);
    let outputs = UserMessageOutputChannels { raw_tx, parsed_tx };

    let (join, handle) = spawn(outputs);
    handle.shutdown();

    let result = timeout(Duration::from_secs(1), join).await;
    assert!(result.is_ok(), "actor should stop after shutdown");
    assert!(result.expect("timeout checked").is_ok());
}
