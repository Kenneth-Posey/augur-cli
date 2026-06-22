//! Interruptible session operation helpers for `CopilotChatActor`.
//!
//! Extracted from `actor.rs` to keep the actor file within the 200-line logic
//! threshold. Covers send and compact operations that race against `Shutdown`
//! commands so the actor stays responsive while waiting for the CLI subprocess.

/// Maximum time to wait for `session.history.compact` to complete.
///
/// Units: Duration (seconds).
/// Rationale: Compaction rewrites the session history on the server side and
/// can take 30-90 seconds on large conversations. 120 seconds gives comfortable
/// headroom without letting a hung compact block the actor indefinitely.
/// Consumers: `compact_or_shutdown`.
pub const COMPACT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Interval between session keepalive pings during idle periods.
///
/// Units: Duration (5 minutes = 300 seconds).
/// Rationale: Server-side sessions can expire in as little as 30-40 minutes of
/// inactivity. A 5-minute interval keeps well inside any observed expiry window
/// at negligible cost - each ping is a single read-only `get_messages()` call
/// to the local CLI subprocess with no CPU overhead.
/// Consumers: `run_command_loop` idle keepalive arm in `actor.rs`.
pub const KEEPALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5 * 60);
/// JSON-RPC internal-error code used by the Copilot server for expired sessions.
pub const JSONRPC_INTERNAL_ERROR: i32 = -32603;

use super::super::commands::CopilotChatCmd;
use crate::actors::copilot::feed_router::{FeedChannels, FeedRouter};
use augur_domain::TokenTrackerHandle;
use tokio::sync::mpsc;

/// Outcome of an interruptible session operation (send or compact).
///
/// Used by `send_or_shutdown` and `compact_or_shutdown` to signal whether the
/// operation completed normally, was aborted by a `Shutdown` command, or failed.
/// Carries the typed `CopilotError` on failure so callers can inspect the
/// JSON-RPC code, message, and optional data payload without relying on string
/// parsing.
/// Consumers: `run_command_loop` `SendMessage` and `Compact` arms.
pub enum SessionOpOutcome {
    /// The operation completed successfully.
    Done,
    /// A `Shutdown` command (or channel close) arrived before completion.
    Shutdown,
    /// The operation failed with the underlying SDK error.
    Error(copilot_sdk::CopilotError),
}

/// Race `session.send(options)` against `cmd_rx` for a `Shutdown` command.
///
/// Pins the send future and loops with a biased `select!` that checks
/// `cmd_rx` first. If `Shutdown` (or channel close) arrives before the send
/// completes, calls `session.abort()` to stop ongoing generation in the CLI
/// subprocess, then returns `SessionOpOutcome::Shutdown`.
///
/// Parameters:
/// - `session`: the active Copilot SDK session.
/// - `options`: the fully-constructed `MessageOptions` to forward to the SDK.
/// - `cmd_rx`: mutable reference to the command receiver, shared with the outer loop.
///
/// Returns:
/// - `Done` when send succeeds (messageId received from CLI).
/// - `Shutdown` when abort was requested; `session.abort()` has been called.
/// - `Error(msg)` when `session.send` returns an error.
///
/// Consumers: `run_command_loop` `SendMessage` arm.
#[tracing::instrument(skip(session, options, cmd_rx), level = "debug")]
pub async fn send_or_shutdown(
    session: &copilot_sdk::Session,
    options: copilot_sdk::MessageOptions,
    cmd_rx: &mut mpsc::Receiver<CopilotChatCmd>,
) -> SessionOpOutcome {
    let send_fut = session.send(options);
    tokio::pin!(send_fut);
    loop {
        tokio::select! {
            biased;
            cmd = cmd_rx.recv() => match cmd {
                None | Some(CopilotChatCmd::Shutdown) => {
                    tracing::info!("CopilotChatActor: shutdown during send, aborting session");
                    let _ = session.abort().await;
                    return SessionOpOutcome::Shutdown;
                }
                Some(_) => {
                    tracing::debug!("CopilotChatActor: command discarded while send in-flight");
                }
            },
            result = &mut send_fut => {
                return match result {
                    Ok(_) => SessionOpOutcome::Done,
                    Err(e) => SessionOpOutcome::Error(e),
                };
            }
        }
    }
}

/// Convert a slice of domain `FilePath` values to Copilot SDK attachment objects.
///
/// Each `FilePath` becomes a `UserMessageAttachment` with `attachment_type: File`,
/// `path` set to the full path string, and `display_name` set to the last path
/// segment (filename). When the path has no segments, `path` is used as the
/// display name.
///
/// This is the single conversion site for `FilePath → UserMessageAttachment`.
/// Callers must not inline this conversion; always call `build_sdk_attachments`.
///
/// Consumers: `run_command_loop` `SendMessage` arm in `actor.rs`.
pub fn build_sdk_attachments(
    paths: &[augur_domain::string_newtypes::FilePath],
) -> Vec<copilot_sdk::UserMessageAttachment> {
    paths
        .iter()
        .map(|p| {
            let path_str: &str = p;
            let display_name = std::path::Path::new(path_str)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path_str)
                .to_owned();
            copilot_sdk::UserMessageAttachment {
                attachment_type: copilot_sdk::AttachmentType::File,
                path: path_str.to_owned(),
                display_name,
            }
        })
        .collect()
}

/// Race `session.compact()` against `cmd_rx` for a `Shutdown` command, with a
/// 120-second timeout applied via `tokio::time::timeout`.
///
/// Pins the compact future wrapped in `tokio::time::timeout(COMPACT_TIMEOUT, …)`
/// and loops with a biased `select!` that checks `cmd_rx` first. If `Shutdown`
/// (or channel close) arrives before the compact completes, calls
/// `session.abort()` and returns `SessionOpOutcome::Shutdown`. If the timeout
/// elapses before completion, returns
/// `SessionOpOutcome::Error(CopilotError::Timeout(COMPACT_TIMEOUT))`.
///
/// Parameters:
/// - `session`: the active Copilot SDK session.
/// - `cmd_rx`: mutable reference to the command receiver, shared with the outer loop.
///
/// Returns:
/// - `Done` when compact succeeds within the timeout window.
/// - `Shutdown` when abort was requested; `session.abort()` has been called.
/// - `Error(CopilotError::Timeout)` when `COMPACT_TIMEOUT` elapses.
/// - `Error(e)` when `session.compact` returns an SDK error.
///
/// Consumers: `run_command_loop` `Compact` arm.
#[tracing::instrument(skip(session, cmd_rx), level = "debug")]
pub async fn compact_or_shutdown(
    session: &copilot_sdk::Session,
    cmd_rx: &mut mpsc::Receiver<CopilotChatCmd>,
) -> SessionOpOutcome {
    let compact_fut = tokio::time::timeout(COMPACT_TIMEOUT, session.compact());
    tokio::pin!(compact_fut);
    loop {
        tokio::select! {
            biased;
            cmd = cmd_rx.recv() => match cmd {
                None | Some(CopilotChatCmd::Shutdown) => {
                    tracing::info!("CopilotChatActor: shutdown during compact, aborting session");
                    let _ = session.abort().await;
                    return SessionOpOutcome::Shutdown;
                }
                Some(_) => {
                    tracing::debug!("CopilotChatActor: command discarded while compact in-flight");
                }
            },
            result = &mut compact_fut => {
                return match result {
                    Err(_elapsed) => {
                        tracing::warn!(timeout_secs = COMPACT_TIMEOUT.as_secs(), "CopilotChatActor: compact timed out");
                        SessionOpOutcome::Error(copilot_sdk::CopilotError::Timeout(COMPACT_TIMEOUT))
                    }
                    Ok(Ok(_)) => SessionOpOutcome::Done,
                    Ok(Err(e)) => SessionOpOutcome::Error(e),
                };
            }
        }
    }
}

/// Owned references to the three dispatch sinks used by [`handle_sdk_event`].
///
/// Bundles `output_tx`, `feed_channels`, and `token_tracker` so that
/// `handle_sdk_event` stays within the 3-parameter limit.
/// Consumers: [`start_event_dispatch`] dispatch loop.
struct EventHandlerCtx<'a> {
    output_tx: &'a tokio::sync::broadcast::Sender<augur_domain::types::AgentOutput>,
    feed_channels: &'a FeedChannels,
    token_tracker: &'a TokenTrackerHandle,
}

/// Process a single successfully received SDK event.
///
/// Logs the event kind, records token usage when present, routes the event
/// through `router`, and forwards any resulting outputs to the appropriate
/// channels.  Called exclusively from the `start_event_dispatch` loop.
async fn handle_sdk_event(
    sdk_event: copilot_sdk::SessionEvent,
    router: &mut FeedRouter,
    ctx: EventHandlerCtx<'_>,
) {
    use crate::actors::copilot::background_event_mapper::extract_llm_usage;
    tracing::info!(
        event_kind = %crate::actors::copilot::feed_router::debug_event_kind(&sdk_event.data),
        "copilot.session_dispatch.sdk_event"
    );
    if let Some(usage) = extract_llm_usage(&sdk_event.data) {
        ctx.token_tracker.record_usage(usage);
    }
    let result = router.route_event(&sdk_event);
    if let Some(out) = result.main_out {
        tracing::info!(out = ?out, "copilot.session_dispatch.main_out");
        if ctx.output_tx.send(out).is_err() {
            tracing::debug!("CopilotChatActor: no subscribers, event dropped");
        }
    }
    if let Some(entry) = result.feed_out {
        tracing::info!(
            feed_id = %crate::actors::copilot::feed_router::debug_feed_id(&entry.feed_id),
            out = ?entry.output,
            "copilot.session_dispatch.feed_out"
        );
        let _ = ctx.feed_channels.send(entry).await;
    }
}

/// Subscribes to SDK session events and routes them through `FeedRouter`.
///
/// Spawns an async task that loops over the session's broadcast event stream.
/// Each event is passed to `router.route_event`, which applies suppression
/// rules and state-machine advances. The `main_out` result is forwarded on
/// `output_tx`; the `feed_out` result is dispatched via `feed_channels.send`.
/// The loop exits when the session event stream closes (`RecvError::Closed`).
/// `RecvError::Lagged` is treated as non-fatal: the loop logs a warning and
/// continues rather than exiting.
///
/// When `args.token_tracker` is set, `AssistantUsage` events are forwarded to
/// the token-tracker actor via `record_usage` so the 1 Hz snapshot ticker can
/// reflect accumulated totals in the status bar.
///
/// Consumers: `run_with_sdk` after successful session creation.
pub fn start_event_dispatch(session: &copilot_sdk::Session, args: EventDispatchArgs) {
    use tokio::sync::broadcast::error::RecvError;
    let EventDispatchArgs {
        output_tx,
        feed_channels,
        token_tracker,
    } = args;
    let mut event_rx = session.subscribe();
    tokio::spawn(async move {
        let mut router = FeedRouter::new();
        loop {
            match event_rx.recv().await {
                Ok(sdk_event) => {
                    let ctx = EventHandlerCtx {
                        output_tx: &output_tx,
                        feed_channels: &feed_channels,
                        token_tracker: &token_tracker,
                    };
                    handle_sdk_event(sdk_event, &mut router, ctx).await;
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!(
                        n,
                        "CopilotChatActor: SDK event receiver lagged, some events missed"
                    );
                }
                Err(RecvError::Closed) => {
                    tracing::debug!(
                        "CopilotChatActor: SDK event channel closed, dispatch loop exiting"
                    );
                    break;
                }
            }
        }
    });
}

/// Arguments bundle for [`start_event_dispatch`].
///
/// Groups the three dispatch outputs so `start_event_dispatch` stays within
/// the 3-parameter limit. Constructed once per session activation.
///
/// Consumers: `activate_session` in `actor.rs`.
#[derive(bon::Builder)]
pub struct EventDispatchArgs {
    /// Broadcast sender for main-conversation `AgentOutput` events.
    pub output_tx: tokio::sync::broadcast::Sender<augur_domain::types::AgentOutput>,
    /// Channel bundle routing `AgentFeedOutput` to the agent-feed or ask panel.
    pub feed_channels: FeedChannels,
    /// Token-tracker handle; receives `record_usage` calls on each `AssistantUsage` event.
    pub token_tracker: TokenTrackerHandle,
}

/// Returns `true` when `e` indicates the server-side session no longer exists.
///
/// Matches two forms observed in production:
/// - `CopilotError::SessionNotFound` - the SDK's explicit session-not-found variant.
/// - `CopilotError::JsonRpc { code: -32603, .. }` where the message contains
///   "session not found" (case-insensitive) - the raw server expiry response.
///
/// All other error variants, including transient RPC failures and timeouts,
/// return `false` so non-fatal errors do not trigger a session restart.
///
/// Consumers: `keepalive_session`.
pub fn is_session_dead(e: &copilot_sdk::CopilotError) -> augur_domain::types::SessionAliveness {
    use augur_domain::types::SessionAliveness;
    use copilot_sdk::CopilotError;
    match e {
        CopilotError::SessionNotFound(_) => SessionAliveness::Dead,
        CopilotError::JsonRpc { code, message, .. } if *code == JSONRPC_INTERNAL_ERROR => {
            if message.to_lowercase().contains("session not found") {
                SessionAliveness::Dead
            } else {
                SessionAliveness::Alive
            }
        }
        _ => SessionAliveness::Alive,
    }
}

/// Send a lightweight keepalive touch to the server session.
///
/// Calls `session.get_messages()` as a read-only operation to keep the server
/// from expiring the session during periods of user inactivity. The response
/// data is discarded - the call is made purely for its side effect of touching
/// the server-side session state.
///
/// Returns:
/// - `true` when the ping succeeded or encountered a transient error; the
///   session is assumed still alive and the caller continues normally.
/// - `false` when the error indicates the session is dead (`is_session_dead`)
///   or the SDK connection is unrecoverable (`is_fatal`). The caller should
///   emit a user notification and trigger a session restart.
///
/// Parameters:
/// - `session`: the active Copilot SDK session.
///
/// Side effects:
/// - Logs `DEBUG` on success; `WARN` on dead session or transient error.
///     - Consumers: `run_command_loop` idle keepalive arm in `actor.rs`.
#[tracing::instrument(skip(session), level = "debug")]
pub async fn keepalive_session(
    session: &copilot_sdk::Session,
) -> augur_domain::types::SessionAliveness {
    use augur_domain::types::SessionAliveness;
    match session.get_messages().await {
        Ok(_) => {
            tracing::debug!("CopilotChatActor: keepalive ping sent");
            SessionAliveness::Alive
        }
        Err(e) if matches!(is_session_dead(&e), SessionAliveness::Dead) || e.is_fatal() => {
            tracing::warn!(error = %e, "CopilotChatActor: keepalive detected dead session");
            SessionAliveness::Dead
        }
        Err(e) => {
            tracing::warn!(error = %e, "CopilotChatActor: keepalive ping failed (transient)");
            SessionAliveness::Alive
        }
    }
}
