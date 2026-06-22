//! Workspace-level crate dependency graph extraction.
//!
//! Uses `cargo_metadata` to resolve the workspace, collect crate nodes
//! and dependency edges, assign topological layers, and extract doc comments.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cargo_metadata::MetadataCommand;

use crate::doc_extractor;
use crate::graph_data::{CrateEdge, CrateNode, WorkspaceGraph};

/// Resolved workspace metadata ready for graph construction.
pub struct ResolvedWorkspace {
    pub graph: WorkspaceGraph,
    /// Map from crate name to its resolved package root path.
    pub crate_paths: HashMap<String, PathBuf>,
}

/// Resolve the workspace at the given manifest path and build the workspace-level graph.
pub fn resolve_workspace(manifest_path: &Path) -> Result<ResolvedWorkspace> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .context("Failed to execute cargo metadata")?;

    // Identify workspace member packages by name.
    let workspace_member_names: HashSet<String> = metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .map(|p| p.name.clone())
        .collect();

    // Collect workspace member packages, excluding integration tests.
    let mut member_names: Vec<String> = Vec::new();
    let mut crate_paths: HashMap<String, PathBuf> = HashMap::new();

    for package in &metadata.packages {
        if workspace_member_names.contains(package.name.as_str())
            && package.name != "augur-integration-tests"
            && package.name != "augur-graph-builder"
        {
            member_names.push(package.name.clone());
            let root = package
                .manifest_path
                .parent()
                .map(|p| PathBuf::from(p.as_str()))
                .unwrap_or_default();
            crate_paths.insert(package.name.clone(), root);
        }
    }

    member_names.sort();
    let member_names_set: HashSet<&str> = member_names.iter().map(|s| s.as_str()).collect();

    // Build a dependency graph (name -> dependency names that are workspace members).
    let mut deps: HashMap<&str, Vec<&str>> = HashMap::new();
    for package in &metadata.packages {
        if !member_names_set.contains(package.name.as_str()) {
            continue;
        }
        let dep_names: Vec<&str> = package
            .dependencies
            .iter()
            .filter_map(|dep| {
                let dep_name = dep.name.as_str();
                if member_names_set.contains(dep_name) {
                    Some(dep_name)
                } else {
                    None
                }
            })
            .collect();
        deps.insert(package.name.as_str(), dep_names);
    }

    // Compute layers via topological sort (longest path from root).
    let layers = compute_layers(&deps, &member_names);

    // Build nodes.
    let mut nodes: Vec<CrateNode> = Vec::new();
    for name in &member_names {
        let doc = extract_crate_doc(name, &crate_paths);
        let layer = layers.get(name.as_str()).copied().unwrap_or(0);
        nodes.push(CrateNode {
            id: name.clone(),
            label: name.clone(),
            doc,
            layer,
        });
    }

    let mut edges: Vec<CrateEdge> = Vec::new();
    for package in &metadata.packages {
        if !member_names_set.contains(package.name.as_str()) {
            continue;
        }
        let target = package.name.as_str();
        for dep in &package.dependencies {
            let dep_name = dep.name.as_str();
            if member_names_set.contains(dep_name) {
                edges.push(CrateEdge {
                    source: dep_name.to_string(),
                    target: target.to_string(),
                });
            }
        }
    }

    Ok(ResolvedWorkspace {
        graph: WorkspaceGraph { nodes, edges },
        crate_paths,
    })
}

/// Compute layer assignments via longest-path topological sort with
/// directed gap-filling.
///
/// Root crates (no workspace dependencies) are assigned layer 0.
/// Each subsequent layer is the longest path from any root to the crate.
/// Then a reverse pass fills gaps: if a crate's minimum consumer layer is
/// more than 1 below its own layer, it gets pushed down to close the gap.
/// This keeps provider crates at the same conceptual layer even when they
/// skip intermediate dependencies. The check uses the *minimum* consumer
/// layer to avoid pulling foundation crates (which feed everything) upward.
fn compute_layers(
    deps: &HashMap<&str, Vec<&str>>,
    member_names: &[String],
) -> HashMap<String, usize> {
    let mut layers: HashMap<String, usize> = HashMap::new();

    // Find root crates (no workspace deps).
    let mut queue: VecDeque<String> = VecDeque::new();
    for name in member_names {
        let dep_list = deps.get(name.as_str()).map(|v| v.as_slice()).unwrap_or(&[]);
        if dep_list.is_empty() {
            layers.insert(name.clone(), 0);
            queue.push_back(name.clone());
        }
    }

    // Forward BFS: propagate layer = max(parent_layer + 1).
    while let Some(current) = queue.pop_front() {
        let current_layer = *layers.get(&current).unwrap_or(&0);
        for name in member_names {
            if let Some(dep_list) = deps.get(name.as_str()) {
                if dep_list.contains(&current.as_str()) {
                    let proposed = current_layer + 1;
                    let existing = layers.get(name).copied().unwrap_or(0);
                    if proposed > existing {
                        layers.insert(name.clone(), proposed);
                        queue.push_back(name.clone());
                    }
                }
            }
        }
    }

    // Assign any remaining crates layer 0.
    for name in member_names {
        layers.entry(name.clone()).or_insert(0);
    }

    // Reverse gap-filling pass: if ALL of a crate's consumers are at layers
    // more than 1 below it, push the crate down to min(consumer_layer) - 1.
    // This fills gaps like copilot-sdk (layer 1) -> app (layer 3) without
    // pulling foundation crates like domain (layer 0) which has consumers at
    // all layers.
    let mut consumers: HashMap<&str, Vec<&str>> = HashMap::new();
    for name in member_names {
        if let Some(dep_list) = deps.get(name.as_str()) {
            for dep in dep_list {
                consumers.entry(dep).or_default().push(name.as_str());
            }
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for name in member_names.iter().rev() {
            let current_layer = *layers.get(name.as_str()).unwrap_or(&0);
            if current_layer == 0 {
                continue;
            } // never push roots

            if let Some(consumer_list) = consumers.get(name.as_str()) {
                // Find the minimum consumer layer
                let mut min_consumer = usize::MAX;
                for consumer in consumer_list {
                    let cl = *layers.get(*consumer).unwrap_or(&0);
                    if cl < min_consumer {
                        min_consumer = cl;
                    }
                }

                if min_consumer != usize::MAX && min_consumer > current_layer + 1 {
                    // Gap detected: push this crate to fill it
                    let new_layer = min_consumer - 1;
                    if new_layer > current_layer {
                        layers.insert(name.clone(), new_layer);
                        changed = true;
                    }
                }
            }
        }
    }

    layers
}

/// Extract the first `//!` doc comment from a crate's src/lib.rs or src/main.rs.
fn extract_crate_doc(crate_name: &str, crate_paths: &HashMap<String, PathBuf>) -> String {
    let Some(root) = crate_paths.get(crate_name) else {
        return String::new();
    };

    // Prefer lib.rs over main.rs.
    let lib_path = root.join("src/lib.rs");
    let main_path = root.join("src/main.rs");

    if lib_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&lib_path) {
            if let Some(doc) = doc_extractor::extract_first_doc_comment(&content) {
                return doc;
            }
        }
    }

    if main_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&main_path) {
            if let Some(doc) = doc_extractor::extract_first_doc_comment(&content) {
                return doc;
            }
        }
    }

    String::new()
}
