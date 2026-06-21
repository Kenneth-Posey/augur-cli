//! ToolActor: receives tool execution commands and dispatches to handlers.

use super::handle::ToolHandle;
use super::tool_actor_ops as actor_ops;
use super::tool_ops::ToolCommand;
use crate::tools::registry::ToolRegistry;
use augur_domain::domain::channels::TOOL_COMMAND_CAPACITY;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Spawn the tool actor and return its join handle plus a communication handle.
///
/// Snapshots the registry's definitions into an immutable Arc for the handle,
/// then wraps the registry itself in Arc for parallel dispatch tasks.
#[tracing::instrument(skip_all, level = "info")]
pub fn spawn(registry: ToolRegistry) -> (JoinHandle<()>, ToolHandle) {
    let definitions = Arc::new(registry.definitions().to_vec());
    let (tx, rx) = mpsc::channel(*TOOL_COMMAND_CAPACITY);
    let handle = ToolHandle::new(tx, Arc::clone(&definitions));
    let join = tokio::spawn(run(registry, rx));
    (join, handle)
}

async fn run(registry: ToolRegistry, mut rx: mpsc::Receiver<ToolCommand>) {
    let registry = Arc::new(registry);
    while let Some(cmd) = rx.recv().await {
        match cmd {
            ToolCommand::Shutdown => break,
            ToolCommand::Execute(tool_cmd) => {
                tokio::spawn(actor_ops::dispatch_tool_call(
                    tool_cmd,
                    Arc::clone(&registry),
                ));
            }
        }
    }
}
