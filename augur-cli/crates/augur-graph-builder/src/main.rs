//! augur-graph-builder — CLI entrypoint.
//!
//! Parses CLI arguments, resolves the workspace, walks the module tree,
//! and writes `graph-data.json` to the specified output path.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use augur_graph_builder::module_walker;
use augur_graph_builder::workspace_graph;

/// CLI arguments.
#[derive(Parser, Debug)]
#[command(name = "augur-graph-builder")]
struct Cli {
    /// Path to the workspace Cargo.toml.
    #[arg(long = "manifest-path")]
    manifest_path: PathBuf,

    /// Output path for graph-data.json.
    #[arg(long = "output")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let manifest_path = cli.manifest_path;
    if !manifest_path.exists() {
        anyhow::bail!("Manifest path does not exist: {}", manifest_path.display());
    }

    // Resolve workspace graph.
    let resolved = workspace_graph::resolve_workspace(&manifest_path)
        .context("Failed to resolve workspace graph")?;

    // Collect workspace crate names.
    let workspace_crate_names: Vec<String> = resolved
        .graph
        .nodes
        .iter()
        .map(|n| n.id.clone())
        .collect();

    // Walk module trees.
    let crate_graphs = module_walker::walk_all_crates(
        &resolved.crate_paths,
        &workspace_crate_names,
    );

    // Build output.
    let output = augur_graph_builder::graph_data::GraphData {
        workspace: resolved.graph,
        crates: crate_graphs,
    };

    // Serialize and write.
    let json = serde_json::to_string_pretty(&output)
        .context("Failed to serialize graph data")?;

    // Ensure parent directory exists.
    if let Some(parent) = cli.output.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create output directory")?;
    }

    std::fs::write(&cli.output, &json)
        .with_context(|| format!("Failed to write output to {}", cli.output.display()))?;

    eprintln!("Graph data written to {}", cli.output.display());
    Ok(())
}