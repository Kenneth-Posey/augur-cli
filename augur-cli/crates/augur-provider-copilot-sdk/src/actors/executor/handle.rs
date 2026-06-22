//! `ExecutorHandle` - cloneable handle to a running `ExecutorActor`.
//!
//! Implements `ExecutorDriver` from `domain::traits` so the supervisor actor
//! depends only on the trait, not on this concrete type. Only `wiring.rs`
//! constructs this handle and passes it to the supervisor.

use super::commands::{ExecutorCmd, ShellExecResult};
use async_trait::async_trait;
use augur_domain::PromptText;
use augur_domain::channels::EXECUTOR_EVENT_BUFFER;
use augur_domain::string_newtypes::{ProcessId, ShellCommand};
use augur_domain::traits::{ExecutorDriver, ExecutorMode};
use augur_domain::types::AgentOutput;
use tokio::sync::{broadcast, mpsc, oneshot};

/// Cloneable handle to a running `ExecutorActor`.
///
/// Wraps the command sender and a broadcast sender for the output stream.
/// All clones share the same underlying channels. Pass to the supervisor via
/// `Box<dyn ExecutorDriver>` so the supervisor is not coupled to this type.
#[derive(Clone)]
pub struct ExecutorHandle {
    cmd_tx: mpsc::Sender<ExecutorCmd>,
    output_tx: broadcast::Sender<AgentOutput>,
}

impl ExecutorHandle {
    /// Construct a handle from raw channel endpoints.
    ///
    /// Called only by `ExecutorActor::spawn`. The `output_tx` is shared with
    /// the actor's event dispatch loop so subscribers receive all events.
    pub(super) fn new(
        cmd_tx: mpsc::Sender<ExecutorCmd>,
        output_tx: broadcast::Sender<AgentOutput>,
    ) -> Self {
        ExecutorHandle { cmd_tx, output_tx }
    }

    /// Execute a shell command through the session synchronously.
    ///
    /// Blocks until the session returns the result. Returns a default
    /// `ShellExecResult` with empty stdout and exit code `1` if the actor
    /// has stopped before the result arrives.
    #[tracing::instrument(skip_all)]
    pub async fn shell_exec(&self, command: ShellCommand) -> ShellExecResult {
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = ExecutorCmd::ShellExec { command, reply_tx };
        let _ = self.cmd_tx.send(cmd).await;
        reply_rx.await.unwrap_or(ShellExecResult {
            process_id: ProcessId::from(""),
        })
    }

    /// Send a graceful stop signal to the actor.
    ///
    /// Uses `try_send`; ignores the error if the actor has already exited.
    pub fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(ExecutorCmd::Stop);
    }
}

#[async_trait]
impl ExecutorDriver for ExecutorHandle {
    /// Send a prompt to the CLI session.
    ///
    /// Uses a lossy `try_send`; logs a warning on channel full.
    #[tracing::instrument(skip_all)]
    async fn send_prompt(&self, content: PromptText) {
        if self
            .cmd_tx
            .send(ExecutorCmd::SendPrompt { content })
            .await
            .is_err()
        {
            tracing::warn!("ExecutorHandle::send_prompt: actor has stopped");
        }
    }

    /// Switch the CLI session into the given mode.
    #[tracing::instrument(skip_all)]
    async fn set_mode(&self, mode: ExecutorMode) {
        if self
            .cmd_tx
            .send(ExecutorCmd::SetMode { mode })
            .await
            .is_err()
        {
            tracing::warn!("ExecutorHandle::set_mode: actor has stopped");
        }
    }

    /// Request conversation compaction from the session.
    #[tracing::instrument(skip_all)]
    async fn compact(&self) {
        if self.cmd_tx.send(ExecutorCmd::Compact).await.is_err() {
            tracing::warn!("ExecutorHandle::compact: actor has stopped");
        }
    }

    /// Subscribe to the executor output broadcast channel.
    ///
    /// Returns a fresh receiver starting from the next emitted event.
    /// The supervisor and TUI call this once at startup to track executor output.
    fn subscribe_output(&self) -> broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }
}

/// Create a broadcast sender/receiver pair for the executor output channel.
///
/// Called by `ExecutorActor::spawn` to build the shared broadcast channel.
/// The sender is stored in the handle; the initial receiver can be dropped
/// since each subscriber calls `subscribe_output` on the handle.
pub(super) fn make_output_channel() -> broadcast::Sender<AgentOutput> {
    let (tx, _) = broadcast::channel(*EXECUTOR_EVENT_BUFFER);
    tx
}
