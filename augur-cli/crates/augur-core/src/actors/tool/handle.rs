//! ToolHandle and ToolExecutor trait for dependency injection.
//!
//! `ToolExecutor` is defined in `domain::traits` and re-exported here for
//! backward compatibility. New code should import from `augur_domain::domain::traits`.

use super::tool_ops::{ToolCall, ToolCallCommand, ToolCommand};
use crate::tools::handler::ToolCallResult;
use augur_domain::tools::definition::ToolDefinition;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub use augur_domain::domain::traits::ToolExecutor;

/// Cloneable handle to a running `ToolActor` task.
///
/// Wraps the command sender plus an immutable Arc snapshot of tool definitions
/// built at spawn time. The `Arc<Vec<_>>` is read-only after construction -
/// it is NOT shared mutable state. Cloning shares the same Arc and channel sender.
#[derive(Clone)]
pub struct ToolHandle {
    tx: mpsc::Sender<ToolCommand>,
    /// Immutable snapshot of all tool schemas, built once at spawn. Read-only.
    definitions: Arc<Vec<ToolDefinition>>,
}

impl ToolHandle {
    /// Create a handle. Called only by `ToolActor::spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<ToolCommand>,
        definitions: Arc<Vec<ToolDefinition>>,
    ) -> Self {
        ToolHandle { tx, definitions }
    }

    /// Send a graceful shutdown signal to the actor.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(ToolCommand::Shutdown);
    }
}

#[async_trait::async_trait]
impl ToolExecutor for ToolHandle {
    fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    #[tracing::instrument(skip(self), fields(tool = %call.name))]
    async fn execute(&self, call: ToolCall) -> anyhow::Result<ToolCallResult> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(ToolCommand::Execute(ToolCallCommand { call, reply_tx }))
            .await
            .map_err(|_| anyhow::anyhow!("tool actor stopped"))?;
        reply_rx
            .await
            .map_err(|_| anyhow::anyhow!("tool actor dropped reply"))
    }
}
