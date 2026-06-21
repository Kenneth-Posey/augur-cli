//! `LspHandle` and `LspRequest`: the public channel surface of the LSP actor.
//!
//! `LspHandle` is the only item from `actors::lsp` consumed by the tools and
//! wiring layers. `LspRequest` is an internal channel-message type scoped to
//! the `actors::lsp` module; external code must not construct it directly.

use augur_domain::domain::lsp::LspError;
use augur_domain::domain::traits::LspClient;
use tokio::sync::{mpsc, oneshot, watch};

/// A single request sent through `LspHandle` to the `LspActor`.
///
/// Carries the JSON-RPC method, parameters, and a one-shot channel
/// through which the actor delivers exactly one response. Consumed
/// exactly once by the actor; `reply_tx` is never cloned.
///
/// # Invariants
///
/// - `reply_tx` is a live, unsent `oneshot::Sender`. It is consumed exactly
///   once by the actor's run loop.
/// - `Send + 'static`: all fields satisfy `Send`.
#[derive(bon::Builder)]
pub(crate) struct LspRequest {
    /// JSON-RPC method string, e.g. `"textDocument/definition"`.
    pub(crate) method: String,
    /// JSON-encoded LSP parameters object.
    pub(crate) params: serde_json::Value,
    /// One-shot sender through which the actor delivers the response.
    ///
    /// Dropped after exactly one send. Never cloned.
    pub(crate) reply_tx: oneshot::Sender<Result<serde_json::Value, LspError>>,
}

/// Cloneable channel-backed reference to the running `LspActor`.
///
/// All clones reach the same actor task. Satisfies `Clone + Send + Sync + 'static`
/// because `mpsc::Sender<T>: Clone + Send + Sync` when `T: Send`, and
/// `LspRequest: Send`. Suitable for storage in `ToolHandler` implementors.
///
/// The embedded `kill_tx` watch channel provides a deterministic shutdown
/// path: calling [`kill`](Self::kill) signals the actor to terminate the
/// rust-analyzer child process and exit, even if the mpsc channel has not
/// yet closed (e.g., during graceful session shutdown).
///
/// # Usage
///
/// ```ignore
/// let (reply_tx, reply_rx) = oneshot::channel();
/// let request = LspRequest { method: "textDocument/definition".into(), params, reply_tx };
/// handle.send(request).await?;
/// let result = tokio::time::timeout(Duration::from_secs(10), reply_rx).await;
/// ```
#[derive(Clone)]
pub struct LspHandle {
    tx: mpsc::Sender<LspRequest>,
    kill_tx: watch::Sender<bool>,
}

impl LspHandle {
    /// Wrap the given mpsc sender and kill watch sender in a new `LspHandle`.
    ///
    /// Called only by [`actors::lsp::actor::spawn`]. The caller must ensure
    /// `tx` is the sender half of a freshly-created mpsc channel paired with
    /// a receiver held by the running `LspActor` task, and that `kill_tx` is
    /// the sender half of a watch channel whose receiver is held by the same
    /// task.
    ///
    /// # Preconditions
    ///
    /// - `tx` is the sender half of a freshly-created mpsc channel.
    /// - `kill_tx` is the sender half of a freshly-created watch channel.
    ///
    /// # Postconditions
    ///
    /// - Returned handle is live; `send()` succeeds until the actor task exits.
    pub(crate) fn new(tx: mpsc::Sender<LspRequest>, kill_tx: watch::Sender<bool>) -> LspHandle {
        LspHandle { tx, kill_tx }
    }

    /// Enqueue an `LspRequest` for processing by the `LspActor`.
    ///
    /// Returns `Ok(())` once the request has been placed into the actor's
    /// channel. Returns `Err(LspError::ProcessDied)` if the channel is closed.
    /// Does **not** impose a timeout; callers must wrap `reply_rx.await` in
    /// `tokio::time::timeout` separately.
    ///
    /// # Preconditions
    ///
    /// - `request.reply_tx` is a live, unsent oneshot sender.
    ///
    /// # Errors
    ///
    /// - `LspError::ProcessDied` - the actor's mpsc channel is closed (actor exited).
    ///
    /// # Postconditions
    ///
    /// - On `Ok(())`: the actor's run loop will receive `request`; the result
    ///   will be delivered via `request.reply_tx`, or dropped if the actor fails.
    ///
    /// # Invariants
    ///
    /// - Does not clone `reply_tx`; the oneshot is consumed exactly once.
    #[allow(private_interfaces)] // LspRequest is pub(crate); LspHandle is re-exported pub(crate)
    pub async fn send(&self, request: LspRequest) -> Result<(), LspError> {
        self.tx
            .send(request)
            .await
            .map_err(|_| LspError::ProcessDied)
    }

    /// Signal the LSP actor to kill the rust-analyzer child process and exit.
    ///
    /// After calling this method the actor transitions to
    /// `LspPhase::Failed(LspError::ProcessDied)`, notifies all pending
    /// callers, and returns from its run loop. This is safe to call
    /// repeatedly and from any thread.
    ///
    /// This is the **graceful-shutdown** kill path. The crash/orphan path
    /// is covered by `PR_SET_PDEATHSIG` (kernel-level death signal) and
    /// `kill_on_drop(true)` on the tokio `Child` handle.
    pub fn kill(&self) {
        let _ = self.kill_tx.send(true);
    }
}

#[async_trait::async_trait]
impl LspClient for LspHandle {
    async fn request(
        &self,
        method: String,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, LspError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let request = LspRequest {
            method,
            params,
            reply_tx,
        };
        self.send(request).await?;
        reply_rx.await.map_err(|_| LspError::ProcessDied)?
    }
}
