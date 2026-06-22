//! Cache actor task: owns the dependency graph, file content, and snapshot.
//!
//! See `handle.rs` for the public interface and `tiers.rs` for tier assignment.

use super::cache_actor_ops as actor_ops;
use crate::actors::cache::cache_ops::{CacheCommand, CacheSnapshot};
use crate::actors::cache::handle::CacheHandle;
use augur_domain::domain::channels::CACHE_COMMAND_CAPACITY;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Human-readable tier label for up to 4 tiers.
pub(super) const TIER_LABELS: [&str; 4] = [
    "Foundation (tier 1)",
    "Core (tier 2)",
    "Context (tier 3)",
    "Working Set (tier 4)",
];

/// Mutable state owned by the cache actor task.
#[derive(bon::Builder)]
pub(super) struct CacheState {
    pub(super) src_dir: PathBuf,
    pub(super) target_file: Option<PathBuf>,
    pub(super) snapshot: Option<CacheSnapshot>,
}

/// Channels and watcher owned by the cache actor task loop.
#[derive(bon::Builder)]
struct CacheActorChannels {
    cmd_rx: mpsc::Receiver<CacheCommand>,
    fs_rx: tokio::sync::mpsc::UnboundedReceiver<PathBuf>,
    watcher: notify::RecommendedWatcher,
}

/// Spawn the cache actor and return a `CacheHandle`.
///
/// The actor watches `src_dir` for `.rs` file changes via `notify`. When a
/// watched file changes, the snapshot is rebuilt if it is in the current
/// transitive closure. `src_dir` should point to the project's `src/` folder.
pub fn spawn(src_dir: PathBuf) -> anyhow::Result<CacheHandle> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<CacheCommand>(*CACHE_COMMAND_CAPACITY);
    let (fs_tx, fs_rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
    let watch_dir = src_dir.clone();
    let mut watcher = build_watcher(fs_tx.clone())?;
    use notify::Watcher;
    watcher.watch(&watch_dir, notify::RecursiveMode::Recursive)?;
    let state = CacheState::builder().src_dir(src_dir).build();
    let channels = CacheActorChannels::builder()
        .cmd_rx(cmd_rx)
        .fs_rx(fs_rx)
        .watcher(watcher)
        .build();
    tokio::spawn(run(state, channels));
    Ok(CacheHandle::new(cmd_tx))
}

fn build_watcher(
    fs_tx: tokio::sync::mpsc::UnboundedSender<PathBuf>,
) -> notify::Result<notify::RecommendedWatcher> {
    notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            actor_ops::forward_rs_paths(event, &fs_tx);
        }
    })
}

/// Main actor loop - drives commands and filesystem events.
async fn run(mut state: CacheState, mut channels: CacheActorChannels) {
    let _watcher = channels.watcher;
    loop {
        tokio::select! {
            Some(cmd) = channels.cmd_rx.recv() => {
                let shutdown = actor_ops::handle_command(cmd, &mut state);
                if bool::from(shutdown) { break; }
            }
            Some(changed) = channels.fs_rx.recv() => {
                actor_ops::handle_file_changed(changed, &mut state);
            }
        }
    }
}

/// Resolve the `src/` directory relative to the project root.
///
/// Returns `root/src`. Does not verify that the directory exists - callers
/// should handle the case where `src_dir` does not yet exist or is empty.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_src_dir(project_root: &Path) -> PathBuf {
    project_root.join("src")
}
