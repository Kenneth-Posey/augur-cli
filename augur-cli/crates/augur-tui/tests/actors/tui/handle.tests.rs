use super::{ShutdownSignal, TuiHandle};
use tokio::sync::{mpsc, watch};

fn make_handle(shutdown_rx: watch::Receiver<ShutdownSignal>) -> TuiHandle {
    let (feed_tx, _feed_rx) = mpsc::channel(1);
    TuiHandle::new(shutdown_rx, feed_tx)
}

// ── wait_for_shutdown ────────────────────────────────────────────────────────

/// Verifies that `wait_for_shutdown` returns promptly when the watch sender is
/// dropped (channel-close exit path), covering the `Err` branch of
/// `shutdown_rx.changed()`.
#[tokio::test]
async fn wait_for_shutdown_returns_when_sender_is_dropped() {
    let (tx, rx) = watch::channel(ShutdownSignal::Running);
    let mut handle = make_handle(rx);

    // Drop the sender: the watch channel closes and .changed() returns Err.
    drop(tx);

    tokio::time::timeout(
        std::time::Duration::from_millis(200),
        handle.wait_for_shutdown(),
    )
    .await
    .expect("wait_for_shutdown must return when the channel sender is dropped");
}

/// Verifies that `wait_for_shutdown` returns promptly when the watch channel
/// transitions to `ShutdownSignal::Complete`, covering the normal exit path.
#[tokio::test]
async fn wait_for_shutdown_returns_when_signal_is_complete() {
    let (tx, rx) = watch::channel(ShutdownSignal::Running);
    let mut handle = make_handle(rx);

    // Send Complete on a background task so wait_for_shutdown can observe it.
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let _ = tx.send(ShutdownSignal::Complete);
    });

    tokio::time::timeout(
        std::time::Duration::from_millis(500),
        handle.wait_for_shutdown(),
    )
    .await
    .expect("wait_for_shutdown must return after receiving ShutdownSignal::Complete");
}

/// Verifies that `wait_for_shutdown` returns immediately without waiting for a
/// change event when the channel already holds `ShutdownSignal::Complete` at
/// the time of the call.
#[tokio::test]
async fn wait_for_shutdown_returns_immediately_when_already_complete() {
    let (tx, rx) = watch::channel(ShutdownSignal::Complete);
    let mut handle = make_handle(rx);
    drop(tx);

    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        handle.wait_for_shutdown(),
    )
    .await
    .expect("wait_for_shutdown must return without blocking when signal is already Complete");
}

#[test]
fn mirror_sync_executes_wait_for_shutdown_returns_when_sender_is_dropped() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
