//! Dependency-graph tier assignment for Anthropic prompt cache injection.
//!
//! Given a flat list of files from a transitive dependency closure and the
//! `DependencyGraph` they came from, assigns each file to a numbered tier
//! (1 = most stable/deep root, N = least stable/closest to target).
//!
//! When the closure has more distinct depth levels than `max_tiers`, all
//! levels shallower than `(depth_count - max_tiers + 1)` are merged into
//! tier 1 so the result always contains at most `max_tiers` groups.

use crate::actors::cache::deps::DependencyGraph;
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use std::collections::HashMap;
use std::path::PathBuf;

/// Assign files in `transitive_deps` to at most `max_tiers` tiers.
///
/// Tier 1 is the most stable (dep-tree roots, depth 0 from the root).
/// The last tier is the least stable (the working target file).
/// When the closure has more distinct depth levels than `max_tiers`, all
/// levels up to `depth_count - max_tiers` are merged into tier 1.
///
/// Returns a `Vec<Vec<PathBuf>>` ordered tier 1 → tier N.
/// Each inner `Vec` may contain multiple files at the same depth level.
pub fn assign_tiers(
    transitive_deps: &[PathBuf],
    graph: &DependencyGraph,
    max_tiers: Count,
) -> Vec<Vec<PathBuf>> {
    if transitive_deps.is_empty() || max_tiers == Count::ZERO {
        return vec![];
    }
    let depth_map = compute_depths(transitive_deps, graph);
    group_by_depth(transitive_deps, &depth_map, max_tiers)
}

/// Compute a depth value for each file in `files`.
///
/// Depth is the length of the longest path from a root (file with no
/// outgoing deps inside the closure) to the file. Roots have depth 0.
/// BFS/Kahn-style: start from roots, propagate depth increments forward.
/// Called once per `assign_tiers` invocation.
fn compute_depths(files: &[PathBuf], graph: &DependencyGraph) -> HashMap<PathBuf, usize> {
    let file_set: std::collections::HashSet<&PathBuf> = files.iter().collect();
    // Build in-closure adjacency: for each file, which closure files does it depend on?
    let mut depth: HashMap<PathBuf, usize> = files.iter().map(|f| (f.clone(), 0)).collect();
    // Iterative relaxation: for each file, depth = max(depth of its deps) + 1.
    // Repeat until stable (handles any DAG depth ≤ file_count passes).
    let passes = files.len();
    for _ in 0..passes {
        let mut changed = false;
        for file in files {
            let max_dep_depth = graph
                .direct_deps(file)
                .iter()
                .filter(|d| file_set.contains(d))
                .filter_map(|d| depth.get(d).copied())
                .max();
            if let Some(d) = max_dep_depth {
                let new_depth = d + 1;
                let entry = depth.entry(file.clone()).or_insert(0);
                if new_depth > *entry {
                    *entry = new_depth;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    depth
}

/// Group files into tier buckets capped at `max_tiers`.
///
/// Depth 0 files → tier 1 (most stable). Deepest files → last tier.
/// When `distinct_levels > max_tiers`, levels `0..collapse_threshold` are
/// merged into tier 1 so the total number of tiers equals `max_tiers`.
/// Called by `assign_tiers` after `compute_depths`.
fn group_by_depth(
    files: &[PathBuf],
    depth_map: &HashMap<PathBuf, usize>,
    max_tiers: Count,
) -> Vec<Vec<PathBuf>> {
    let max_depth = depth_map.values().copied().max().unwrap_or(0);
    let distinct_levels = max_depth + 1;
    let layout = TierLayout::builder()
        .collapse_threshold(distinct_levels.saturating_sub(max_tiers.inner()))
        .distinct_levels(distinct_levels)
        .max_tiers(max_tiers)
        .build();
    let mut buckets: Vec<Vec<PathBuf>> = std::iter::repeat_with(Vec::new)
        .take(max_tiers.inner().min(distinct_levels))
        .collect();
    for file in files {
        let depth = depth_map.get(file).copied().unwrap_or(0);
        let tier_idx = compute_tier_idx(depth, &layout);
        if tier_idx < buckets.len() {
            buckets[tier_idx].push(file.clone());
        }
    }
    buckets.retain(|b| !b.is_empty());
    buckets
}

/// Map a raw depth value to a 0-based tier index.
///
/// Uses `effective_tiers = min(max_tiers, distinct_levels)` so the returned
/// index is always a valid bucket index. Depths below `collapse_threshold` all
/// map to tier 0 (merged into tier 1). Depths from `collapse_threshold` onward
/// map to consecutive tier indices proportionally scaled to `effective_tiers`.
#[derive(bon::Builder)]
struct TierLayout {
    collapse_threshold: usize,
    distinct_levels: usize,
    max_tiers: Count,
}

fn compute_tier_idx(depth: usize, layout: &TierLayout) -> usize {
    let effective_tiers = layout.max_tiers.inner().min(layout.distinct_levels);
    if depth < layout.collapse_threshold {
        return 0;
    }
    let shifted = depth - layout.collapse_threshold;
    let total_shifted = layout.distinct_levels - layout.collapse_threshold;
    if total_shifted <= 1 || effective_tiers <= 1 {
        return 0;
    }
    let idx = shifted * (effective_tiers - 1) / (total_shifted - 1);
    idx.min(effective_tiers - 1)
}
