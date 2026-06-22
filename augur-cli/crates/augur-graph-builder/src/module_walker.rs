//! Intra-crate module tree walker.
//!
//! Walks `mod.rs` files within a crate to build the module tree, collecting
//! intra-crate and cross-crate dependency edges.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use syn::{Item, UseTree};

/// Shared mutable context for recursive module tree collection.
///
/// Bundles multiple parameters that are passed through to every recursive
/// call of `collect_module_tree`, keeping function signatures compact.
struct ModuleTreeContext<'a> {
    src_dir: &'a Path,
    crate_name: &'a str,
    workspace_crate_names: &'a [String],
    /// Maps underscore crate names (augur_domain) back to canonical hyphen form (augur-domain).
    crate_name_map: &'a HashMap<String, String>,
    nodes: &'a mut Vec<ModuleNode>,
    edges: &'a mut Vec<ModuleEdge>,
    cross_edges: &'a mut Vec<CrossCrateEdge>,
}

use crate::doc_extractor;
use crate::graph_data::{CrateModuleGraph, CrossCrateEdge, ModuleEdge, ModuleNode};
use crate::symbol_extractor;

/// Walk all workspace crates and produce per-crate module graphs.
pub fn walk_all_crates(
    crate_paths: &HashMap<String, std::path::PathBuf>,
    workspace_crate_names: &[String],
) -> HashMap<String, CrateModuleGraph> {
    let mut result: HashMap<String, CrateModuleGraph> = HashMap::new();

    for (crate_name, root) in crate_paths {
        let src_dir = root.join("src");
        let lib_rs = src_dir.join("lib.rs");
        let main_rs = src_dir.join("main.rs");

        let root_file = if lib_rs.exists() {
            lib_rs
        } else if main_rs.exists() {
            main_rs
        } else {
            eprintln!("[skip] {}: no src/lib.rs or src/main.rs found", crate_name);
            continue;
        };

        match walk_crate(crate_name, &root_file, workspace_crate_names) {
            Ok(graph) => {
                result.insert(crate_name.clone(), graph);
            }
            Err(e) => {
                eprintln!("[skip] {}: failed to walk module tree: {}", crate_name, e);
            }
        }
    }

    result
}

/// Walk a single crate's module tree starting from its root source file.
fn walk_crate(
    crate_name: &str,
    root_file: &Path,
    workspace_crate_names: &[String],
) -> Result<CrateModuleGraph> {
    let root_dir = root_file.parent().unwrap_or(root_file);
    let mut nodes: Vec<ModuleNode> = Vec::new();
    let mut edges: Vec<ModuleEdge> = Vec::new();
    let mut cross_edges: Vec<CrossCrateEdge> = Vec::new();

    let source = std::fs::read_to_string(root_file)?;
    let syntax_tree: syn::File = syn::parse_file(&source)?;

    let root_doc = doc_extractor::extract_first_doc_comment(&source).unwrap_or_default();
    let root_module_name = module_name_from_file(root_file);

    let root_id = format!("{}::{}", crate_name, root_module_name);
    let root_children = collect_child_modules(root_dir, &syntax_tree, crate_name, root_dir);

    nodes.push(ModuleNode {
        id: root_id.clone(),
        label: root_module_name.to_string(),
        doc: root_doc,
        visibility: "pub".to_string(),
        children: root_children.iter().map(|c| c.id.clone()).collect(),
        symbols: symbol_extractor::extract_symbols(&source),
    });

    // Normalize: workspace crate names use hyphens (augur-domain) in Cargo.toml
    // but Rust `use` statements use underscores (augur_domain). We need both forms
    // so process_use_tree can match against either.
    let ws_names_normalized: HashSet<String> = workspace_crate_names
        .iter()
        .flat_map(|s| vec![s.clone(), s.replace('-', "_")])
        .collect();
    let ws_normalized_refs: HashSet<&str> =
        ws_names_normalized.iter().map(|s| s.as_str()).collect();

    // Build a map from underscore form to canonical hyphen form for cross-crate edges.
    let crate_name_map: HashMap<String, String> = workspace_crate_names
        .iter()
        .flat_map(|s| vec![(s.replace('-', "_"), s.clone()), (s.clone(), s.clone())])
        .collect();

    collect_use_edges(
        &source,
        crate_name,
        &root_id,
        &ws_normalized_refs,
        &crate_name_map,
        &mut edges,
        &mut cross_edges,
    );

    // Also scan all sibling .rs files in the crate root for use edges.
    if let Ok(entries) = std::fs::read_dir(root_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && path.file_name().and_then(|n| n.to_str()) != Some("lib.rs")
                && path.file_name().and_then(|n| n.to_str()) != Some("main.rs")
            {
                if let Ok(src) = std::fs::read_to_string(&path) {
                    collect_use_edges(
                        &src,
                        crate_name,
                        &root_id,
                        &ws_normalized_refs,
                        &crate_name_map,
                        &mut edges,
                        &mut cross_edges,
                    );
                }
            }
        }
    }

    let mut ctx = ModuleTreeContext {
        src_dir: root_dir,
        crate_name,
        workspace_crate_names,
        crate_name_map: &crate_name_map,
        nodes: &mut nodes,
        edges: &mut edges,
        cross_edges: &mut cross_edges,
    };

    for child in root_children {
        collect_module_tree(&child, &mut ctx);
    }

    // Filter out edges that reference nodes not in the node list.
    // This handles the case where a `use crate::foo::bar::Baz` references
    // a module path (foo::bar) that isn't a mod.rs-based module.
    let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    edges.retain(|e| node_ids.contains(e.source.as_str()) && node_ids.contains(e.target.as_str()));
    cross_edges.retain(|ce| node_ids.contains(ce.source.as_str()));

    Ok(CrateModuleGraph {
        nodes,
        edges,
        cross_edges,
    })
}

/// Recursively collect module tree starting from a child module descriptor.
fn collect_module_tree(desc: &ModuleDesc, ctx: &mut ModuleTreeContext<'_>) {
    let source = match std::fs::read_to_string(&desc.mod_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[skip] {}: cannot read {}: {}",
                ctx.crate_name,
                desc.mod_path.display(),
                e
            );
            return;
        }
    };

    let syntax_tree: syn::File = match syn::parse_file(&source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "[skip] {}: cannot parse {}: {}",
                ctx.crate_name,
                desc.mod_path.display(),
                e
            );
            return;
        }
    };

    let doc = doc_extractor::extract_first_doc_comment(&source).unwrap_or_default();

    let mod_dir = desc.mod_path.parent().unwrap_or(ctx.src_dir);
    let children = collect_child_modules(mod_dir, &syntax_tree, ctx.crate_name, ctx.src_dir);
    let child_ids: Vec<String> = children.iter().map(|c| c.id.clone()).collect();

    // Collect symbols from all .rs files in this module's directory
    let mut symbols: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(mod_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                if let Ok(src) = std::fs::read_to_string(&path) {
                    symbols.extend(symbol_extractor::extract_symbols(&src));
                }
            }
        }
    }

    ctx.nodes.push(ModuleNode {
        id: desc.id.clone(),
        label: desc.label.clone(),
        doc,
        visibility: "pub".to_string(),
        children: child_ids,
        symbols,
    });

    // Normalize for hyphens vs underscores: workspace crate names use hyphens
    // (augur-domain) in Cargo.toml but Rust `use` statements use underscores
    // (augur_domain). We include both forms so process_use_tree matches either.
    let ws_names_normalized: HashSet<String> = ctx
        .workspace_crate_names
        .iter()
        .flat_map(|s| vec![s.clone(), s.replace('-', "_")])
        .collect();
    let ws_normalized_refs: HashSet<&str> =
        ws_names_normalized.iter().map(|s| s.as_str()).collect();

    // Scan mod.rs for use edges
    collect_use_edges(
        &source,
        ctx.crate_name,
        &desc.id,
        &ws_normalized_refs,
        ctx.crate_name_map,
        ctx.edges,
        ctx.cross_edges,
    );

    // Also scan all sibling .rs files in this module's directory for use edges.
    // These files (e.g. agent_actor.rs, agent_ops.rs) contain the majority of
    // `use crate::` and `use <crate>::` statements in the crate.
    if let Ok(entries) = std::fs::read_dir(mod_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && path.file_name().and_then(|n| n.to_str()) != Some("mod.rs")
            {
                if let Ok(src) = std::fs::read_to_string(&path) {
                    collect_use_edges(
                        &src,
                        ctx.crate_name,
                        &desc.id,
                        &ws_normalized_refs,
                        ctx.crate_name_map,
                        ctx.edges,
                        ctx.cross_edges,
                    );
                }
            }
        }
    }

    for child in children {
        collect_module_tree(&child, ctx);
    }
}

/// Descriptor for a discovered module.
struct ModuleDesc {
    /// Canonical module path (e.g. `augur-core::actors::tool`).
    id: String,
    /// Short module name (e.g. `tool`).
    label: String,
    /// Filesystem path to the mod.rs file.
    mod_path: std::path::PathBuf,
}

/// Extract the module name from a root file path (lib.rs or main.rs).
fn module_name_from_file(path: &Path) -> &str {
    match path.file_name().and_then(|n| n.to_str()) {
        Some("lib.rs") => "lib",
        Some("main.rs") => "crate",
        _ => "unknown",
    }
}

/// Collect child module descriptors from a `mod.rs` file.
fn collect_child_modules(
    mod_dir: &Path,
    syntax_tree: &syn::File,
    crate_name: &str,
    src_dir: &Path,
) -> Vec<ModuleDesc> {
    let mut children: Vec<ModuleDesc> = Vec::new();

    for item in &syntax_tree.items {
        if let Item::Mod(mod_item) = item {
            if mod_item.semi.is_none() {
                continue;
            }
            let mod_name = mod_item.ident.to_string();
            let is_pub = matches!(mod_item.vis, syn::Visibility::Public(_));

            if has_path_attribute(&mod_item.attrs) {
                eprintln!(
                    "[skip] {}: {} has #[path] attribute, skipped",
                    crate_name, mod_name
                );
                continue;
            }

            if !is_pub {
                continue;
            }

            let child_mod_rs = mod_dir.join(&mod_name).join("mod.rs");
            if child_mod_rs.exists() {
                let rel_path = child_mod_rs.strip_prefix(src_dir).unwrap_or(&child_mod_rs);
                let canonical = build_canonical_path(crate_name, rel_path);

                children.push(ModuleDesc {
                    id: canonical,
                    label: mod_name,
                    mod_path: child_mod_rs,
                });
            }
        }
    }

    children
}

/// Check if an attribute list contains a `#[path = "..."]` attribute.
fn has_path_attribute(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().get_ident().is_some_and(|id| id == "path") {
            return true;
        }
    }
    false
}

/// Build a canonical module path from a relative path to a mod.rs.
fn build_canonical_path(crate_name: &str, rel_path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(crate_name.to_string());

    for component in rel_path.components() {
        if let std::path::Component::Normal(s) = component {
            let s = s.to_string_lossy();
            if s == "mod.rs" || s == "src" {
                continue;
            }
            parts.push(s.to_string());
        }
    }

    parts.join("::")
}

/// Collect `use crate::` and `use <workspace-crate>::` edges from source.
fn collect_use_edges(
    source: &str,
    current_crate: &str,
    module_id: &str,
    workspace_crate_names: &HashSet<&str>,
    crate_name_map: &HashMap<String, String>,
    edges: &mut Vec<ModuleEdge>,
    cross_edges: &mut Vec<CrossCrateEdge>,
) {
    let syntax_tree: syn::File = match syn::parse_file(source) {
        Ok(f) => f,
        Err(_) => return,
    };

    for item in &syntax_tree.items {
        if let Item::Use(item_use) = item {
            process_use_tree(
                &item_use.tree,
                current_crate,
                module_id,
                workspace_crate_names,
                crate_name_map,
                edges,
                cross_edges,
            );
        }
    }
}

/// Process a single `use` tree, extracting intra- and cross-crate edges.
fn process_use_tree(
    tree: &UseTree,
    current_crate: &str,
    module_id: &str,
    workspace_crate_names: &HashSet<&str>,
    crate_name_map: &HashMap<String, String>,
    edges: &mut Vec<ModuleEdge>,
    cross_edges: &mut Vec<CrossCrateEdge>,
) {
    match tree {
        UseTree::Path(use_path) => {
            let mut path_prefix = vec![use_path.ident.to_string()];
            path_prefix.extend(collect_chain_prefix(&use_path.tree));

            if path_prefix.is_empty() {
                return;
            }

            let first = &path_prefix[0];

            if first == "crate" && path_prefix.len() >= 2 {
                // Derive the containing module path. The last segment is the imported
                // item (type, function, etc.), so we strip it to find the module.
                // `use crate::Foo` → root module (lib/crate)
                // `use crate::foo::Bar` → crate::foo
                let root_module = module_id.split("::").nth(1).unwrap_or("lib");
                let target_id = if path_prefix.len() == 2 {
                    format!("{}::{}", current_crate, root_module)
                } else {
                    let target_module = path_prefix[1..path_prefix.len() - 1].join("::");
                    format!("{}::{}", current_crate, target_module)
                };
                if target_id != *module_id {
                    edges.push(ModuleEdge {
                        source: target_id,
                        target: module_id.to_string(),
                    });
                }
                return;
            }

            if workspace_crate_names.contains(first.as_str()) && path_prefix.len() >= 2 {
                // Map back from underscore form to canonical hyphenated crate name.
                let target_crate = crate_name_map.get(first.as_str()).unwrap_or(first);
                let target_module = if path_prefix.len() == 2 {
                    // `use augur_core::Foo` → containing module is crate root (lib).
                    format!("{}::lib", target_crate)
                } else {
                    // `use augur_core::foo::Bar` → containing module is `foo`.
                    // The crate name (first) is already tracked in target_crate,
                    // so we use only the path segments between crate and item.
                    let module = path_prefix[1..path_prefix.len() - 1].join("::");
                    format!("{}::{}", target_crate, module)
                };
                cross_edges.push(CrossCrateEdge {
                    source: module_id.to_string(),
                    target_crate: target_crate.clone(),
                    target_module,
                });
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                process_use_tree(
                    item,
                    current_crate,
                    module_id,
                    workspace_crate_names,
                    crate_name_map,
                    edges,
                    cross_edges,
                );
            }
        }
        UseTree::Rename(rename) => {
            let name = rename.ident.to_string();
            if workspace_crate_names.contains(name.as_str()) {
                let target = name.clone() + "::lib";
                cross_edges.push(CrossCrateEdge {
                    source: module_id.to_string(),
                    target_crate: name,
                    target_module: target,
                });
            }
        }
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if workspace_crate_names.contains(ident.as_str()) {
                let target = ident.clone() + "::lib";
                cross_edges.push(CrossCrateEdge {
                    source: module_id.to_string(),
                    target_crate: ident,
                    target_module: target,
                });
            }
        }
        UseTree::Glob(_) => {}
    }
}

/// Recursively collect path prefix names from a chain of UseTree nodes.
fn collect_chain_prefix(tree: &UseTree) -> Vec<String> {
    match tree {
        UseTree::Path(use_path) => {
            let mut segments = vec![use_path.ident.to_string()];
            segments.extend(collect_chain_prefix(&use_path.tree));
            segments
        }
        UseTree::Name(name) => vec![name.ident.to_string()],
        UseTree::Rename(rename) => vec![rename.ident.to_string()],
        UseTree::Glob(_) => vec!["*".to_string()],
        UseTree::Group(_) => vec![],
    }
}
