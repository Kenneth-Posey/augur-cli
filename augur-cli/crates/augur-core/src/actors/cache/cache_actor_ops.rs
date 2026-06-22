//! Private helper operations for the cache actor.

use crate::actors::cache::cache_actor::CacheState;
use crate::actors::cache::cache_actor::TIER_LABELS;
use crate::actors::cache::cache_ops::CacheCommand;
use crate::actors::cache::cache_ops::CacheSnapshot;
use crate::actors::cache::cache_ops::{CachedFile, CachedTier};
use crate::actors::cache::deps::DependencyGraph;
use crate::actors::cache::tiers::assign_tiers;
use augur_domain::domain::newtypes::{Count, IsPredicate};
use augur_domain::domain::string_newtypes::{StatusLabel, StringNewtype};
use std::path::PathBuf;

/// Forward only changed Rust source paths from a notify event.
pub(super) fn forward_rs_paths(
    event: notify::Event,
    fs_tx: &tokio::sync::mpsc::UnboundedSender<PathBuf>,
) {
    for path in event.paths {
        if path.extension().is_some_and(|extension| extension == "rs") {
            let _ = fs_tx.send(path);
        }
    }
}

/// Convert tier file-path groups into `CachedTier` structs by reading content.
pub(super) fn build_tiers(tier_groups: Vec<Vec<PathBuf>>) -> Vec<CachedTier> {
    tier_groups
        .into_iter()
        .enumerate()
        .map(|(i, paths)| build_single_tier(i, paths))
        .collect()
}

/// Dispatch a single cache command and return `true` when the actor should stop.
pub(super) fn handle_command(cmd: CacheCommand, state: &mut CacheState) -> IsPredicate {
    match cmd {
        CacheCommand::SetWorkingFile(path) => {
            state.target_file = Some(path);
            rebuild_snapshot(state);
            IsPredicate::no()
        }
        CacheCommand::RefreshFile(_path) => {
            rebuild_snapshot(state);
            IsPredicate::no()
        }
        CacheCommand::GetSnapshot(tx) => {
            let _ = tx.send(state.snapshot.clone());
            IsPredicate::no()
        }
        CacheCommand::Shutdown => IsPredicate::yes(),
    }
}

/// Handle one filesystem change event and rebuild when the snapshot includes it.
pub(super) fn handle_file_changed(changed: PathBuf, state: &mut CacheState) {
    let is_in_snapshot = state.snapshot.as_ref().is_some_and(|snapshot| {
        snapshot
            .tiers
            .iter()
            .any(|tier| tier.files.iter().any(|file| file.path == changed))
    });
    if is_in_snapshot {
        rebuild_snapshot(state);
    }
}

/// Rebuild snapshot tiers from the current target file dependency closure.
fn rebuild_snapshot(state: &mut CacheState) {
    let target = match &state.target_file {
        Some(path) => path.clone(),
        None => {
            state.snapshot = None;
            return;
        }
    };
    let graph = match DependencyGraph::from_src_dir(&state.src_dir) {
        Ok(graph) => graph,
        Err(error) => {
            tracing::warn!(error = %error, "cache: failed to build dep graph");
            state.snapshot = None;
            return;
        }
    };
    let deps = graph.transitive_deps(&target);
    let tier_groups = assign_tiers(&deps, &graph, Count::of(4));
    let tiers = build_tiers(tier_groups);
    state.snapshot = Some(CacheSnapshot { tiers });
}

fn build_single_tier(index: usize, paths: Vec<PathBuf>) -> CachedTier {
    let label = StatusLabel::new(TIER_LABELS.get(index).copied().unwrap_or("Tier"));
    let files = paths.into_iter().filter_map(read_cached_file).collect();
    CachedTier { label, files }
}

fn read_cached_file(path: PathBuf) -> Option<CachedFile> {
    match std::fs::read_to_string(&path) {
        Ok(content) => Some(CachedFile {
            path,
            content: content.into(),
        }),
        Err(e) => {
            tracing::debug!(path = %path.display(), error = %e, "cache: skipping unreadable file");
            None
        }
    }
}
