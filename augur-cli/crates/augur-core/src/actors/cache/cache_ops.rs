//! Cache actor domain types: cached file content, tier groupings, and commands.

use std::path::PathBuf;
use tokio::sync::oneshot;

pub use augur_domain::domain::types::{CacheSnapshot, CachedFile, CachedTier};

/// Commands sent to the cache actor via `CacheHandle`.
///
/// Each variant carries data needed by the actor to update state or reply.
pub enum CacheCommand {
    /// Set the file currently being worked on. Triggers a full dep-graph
    /// analysis and snapshot rebuild from the transitive dependency closure.
    SetWorkingFile(PathBuf),
    /// Force a re-read of `path` and rebuild the snapshot. Used by the
    /// `refresh_cache_file` tool when the LLM wants updated file content.
    RefreshFile(PathBuf),
    /// Request the current snapshot. Sends `None` if no working file is set.
    GetSnapshot(oneshot::Sender<Option<CacheSnapshot>>),
    /// Gracefully shut down the actor task loop.
    Shutdown,
}
