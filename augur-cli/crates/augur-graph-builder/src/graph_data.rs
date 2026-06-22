//! Graph data types matching the output schema from the plan.
//!
//! These types serialize to the JSON structure consumed by the HTML viewer.

use serde::{Deserialize, Serialize};

/// Top-level graph data emitted by the builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub workspace: WorkspaceGraph,
    pub crates: std::collections::HashMap<String, CrateModuleGraph>,
}

/// Workspace-level crate dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceGraph {
    pub nodes: Vec<CrateNode>,
    pub edges: Vec<CrateEdge>,
}

/// A single workspace crate node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub doc: String,
    pub layer: usize,
}

/// A directed dependency edge between workspace crates.
/// Direction: source (depended-on) → target (depending).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateEdge {
    pub source: String,
    pub target: String,
}

/// Module-level graph for a single crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateModuleGraph {
    pub nodes: Vec<ModuleNode>,
    pub edges: Vec<ModuleEdge>,
    pub cross_edges: Vec<CrossCrateEdge>,
}

/// A single module node within a crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub doc: String,
    pub visibility: String,
    #[serde(default)]
    pub children: Vec<String>,
    /// Top-level symbols (functions, types, traits, constants) declared in this module.
    #[serde(default)]
    pub symbols: Vec<String>,
}

/// An intra-crate dependency edge between modules.
/// Direction: source (depended-on) → target (depending).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleEdge {
    pub source: String,
    pub target: String,
}

/// A cross-crate dependency edge from a module to a workspace-crate module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossCrateEdge {
    pub source: String,
    pub target_crate: String,
    pub target_module: String,
}
