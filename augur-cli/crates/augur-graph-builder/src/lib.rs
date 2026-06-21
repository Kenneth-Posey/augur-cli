//! augur-graph-builder — Workspace dependency graph extraction tool.
//!
//! This crate analyzes a Cargo workspace to produce a structured JSON
//! representation of the crate dependency graph and intra-crate module trees.
//! The output is consumed by the interactive graph viewer in `public-html/`.

pub mod doc_extractor;
pub mod graph_data;
pub mod module_walker;
pub mod workspace_graph;pub mod symbol_extractor;