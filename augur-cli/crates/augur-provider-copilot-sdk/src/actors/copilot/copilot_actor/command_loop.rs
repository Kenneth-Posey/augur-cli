use super::super::assistant::KEEPALIVE_INTERVAL;
use super::runtime_types::{CommandLoopState, CopilotCmdContext, LoopExit};

/// Process `CopilotChatCmd` messages until `Shutdown`, channel close, or send error.
///
/// Simultaneously monitors the output broadcast channel to accumulate assistant
/// tokens for logging. When `TurnComplete` is received, logs the user prompt
/// and the assembled assistant response via the logger handle.
///
/// `session.send` and `session.compact` are both interruptible: if `Shutdown`
/// arrives while either is in-flight, the loop returns immediately rather than
/// waiting for the CLI subprocess to respond.
///
/// The outer `select!` is biased to prefer `log_rx` over `cmd_rx`. This
/// ensures a buffered `TurnComplete` from the previous turn is processed before
/// a new `SendMessage` mutates `pending_user` and `assistant_buf`. Both the
/// `SendMessage` and `Compact` arms call `drain_log_events` as a second safety:
/// any events that were buffered during the previous operation are flushed
/// before the new turn's state is installed.
///
/// Returns `LoopExit::Clean` when the loop exited cleanly (Shutdown or channel closed).
/// Returns `LoopExit::FatalError` when a fatal session send error occurred - the
/// caller should attempt one session restart before giving up.
/// Returns `LoopExit::ReplaceSession(id)` when the TUI requested a new or resumed
/// SDK session; the caller must create/resume the session and re-enter the loop.
/// Consumers: `run_with_sdk` after session creation and event loop spawn.
pub(super) async fn run_command_loop(
    session: &copilot_sdk::Session,
    ctx: &mut CopilotCmdContext<'_>,
) -> LoopExit {
    use tokio::time::MissedTickBehavior;

    let mut log_rx = ctx.output_tx.subscribe();
    let keepalive_start = tokio::time::Instant::now() + KEEPALIVE_INTERVAL;
    let mut keepalive_tick = tokio::time::interval_at(keepalive_start, KEEPALIVE_INTERVAL);
    keepalive_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            out = log_rx.recv() => {
                super::command_handlers::handle_log_output(out, &mut ctx.log).await;
            }
            cmd = ctx.cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                if let Some(exit) = super::command_handlers::handle_loop_command(
                    CommandLoopState::builder()
                        .session(session)
                        .ctx(ctx)
                        .log_rx(&mut log_rx)
                        .build(),
                    cmd,
                )
                .await
                {
                    return exit;
                }
            }
            _ = keepalive_tick.tick() => {
                if let Some(exit) = super::command_handlers::handle_keepalive_tick(session, ctx.output_tx).await {
                    return exit;
                }
            }
        }
    }
    LoopExit::Clean
}
