//! `CacheHandle` - typed interface to the cache actor task.

use crate::actors::cache::cache_ops::{CacheCommand, CacheSnapshot};
use crate::tools::ports::CacheToolPort;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

/// Handle for communicating with the cache actor from other actors or tools.
///
/// All methods are async and return `anyhow::Result` - a closed channel (actor
/// stopped) produces an error rather than a panic. Clone to share across tasks.
#[derive(Clone)]
pub struct CacheHandle {
    tx: mpsc::Sender<CacheCommand>,
}

impl CacheHandle {
    /// Construct a handle wrapping the given command sender.
    ///
    /// Called exclusively by `CacheActor::spawn`; not for external use.
    pub(crate) fn new(tx: mpsc::Sender<CacheCommand>) -> Self {
        Self { tx }
    }

    /// Tell the cache actor which file is currently being worked on.
    ///
    /// Triggers a full dep-graph analysis and snapshot rebuild from the
    /// transitive dependency closure of `path`. Call when the user or LLM
    /// identifies a target file for an editing session.
    #[tracing::instrument(skip(self), err)]
    pub async fn set_working_file(&self, path: PathBuf) -> anyhow::Result<()> {
        self.tx
            .send(CacheCommand::SetWorkingFile(path))
            .await
            .map_err(|_| anyhow::anyhow!("cache actor has stopped"))
    }

    /// Force a re-read of `path` and rebuild the snapshot.
    ///
    /// Use when the LLM knows a file has changed and wants updated context in
    /// the next request. Corresponds to the `refresh_cache_file` tool.
    #[tracing::instrument(skip(self), err)]
    pub async fn refresh_file(&self, path: PathBuf) -> anyhow::Result<()> {
        self.tx
            .send(CacheCommand::RefreshFile(path))
            .await
            .map_err(|_| anyhow::anyhow!("cache actor has stopped"))
    }

    /// Return the current `CacheSnapshot`, or `None` if no working file is set.
    ///
    /// Called by the agent actor before each LLM request to inject tiered file
    /// content into the Anthropic system message. Returns `None` until
    /// `set_working_file` has been called at least once.
    #[tracing::instrument(skip(self), err)]
    pub async fn get_snapshot(&self) -> anyhow::Result<Option<CacheSnapshot>> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(CacheCommand::GetSnapshot(reply_tx))
            .await
            .map_err(|_| anyhow::anyhow!("cache actor has stopped"))?;
        reply_rx
            .await
            .map_err(|_| anyhow::anyhow!("cache actor dropped reply"))
    }

    /// Send a shutdown command to the actor.
    ///
    /// The actor task exits cleanly after processing any in-flight commands.
    /// Subsequent calls on this handle will return errors.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(CacheCommand::Shutdown);
    }
}

#[async_trait::async_trait]
impl CacheToolPort for CacheHandle {
    async fn set_working_file(&self, path: PathBuf) -> anyhow::Result<()> {
        CacheHandle::set_working_file(self, path).await
    }

    async fn refresh_file(&self, path: PathBuf) -> anyhow::Result<()> {
        CacheHandle::refresh_file(self, path).await
    }
}
