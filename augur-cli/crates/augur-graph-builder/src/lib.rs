//! augur-graph-builder - Workspace dependency graph extraction tool.
//!
//! This crate analyzes a Cargo workspace to produce a structured JSON
//! representation of the crate dependency graph and intra-crate module trees.
//! The output is consumed by the interactive graph viewer in `public-html/`.

extern crate self as augur_graph_builder;
pub mod doc_extractor;
pub mod graph_data;
pub mod module_walker;
pub mod symbol_extractor;
pub mod workspace_graph;

// ── Test discovery stubs (rust-analyzer visibility) ────────────────────────
// Makes VS Code / rust-analyzer discover tests in the external tests/
// directory.  Without these, tests declared only through [[test]] in
// Cargo.toml are invisible to the IDE test explorer.
#[cfg(test)]
#[path = "../tests/doc_extractor.tests.rs"]
mod doc_extractor_tests;
#[cfg(test)]
#[path = "../tests/module_walker.tests.rs"]
mod module_walker_tests;
#[cfg(test)]
#[path = "../tests/symbol_extractor.tests.rs"]
mod symbol_extractor_tests;
#[cfg(test)]
#[path = "../tests/workspace_graph.tests.rs"]
mod workspace_graph_tests;
