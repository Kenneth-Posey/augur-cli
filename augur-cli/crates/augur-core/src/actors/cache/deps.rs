//! Intra-project Rust dependency graph parser.
//!
//! Scans `.rs` files under a `src_dir`, extracts `use crate::` imports and
//! `mod name;` declarations, and resolves them to concrete file paths within
//! the same project. The result is a directed graph: file → files it depends on.
//!
//! Only intra-project dependencies are tracked. External crate imports and
//! `use super::` / `use self::` paths are skipped.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Intra-project dependency graph for Rust source files.
///
/// `edges` maps each scanned file to the list of intra-project files it
/// directly depends on. Files with no resolvable deps map to an empty vec.
/// Built once from the `src_dir`; refresh by constructing a new instance.
pub struct DependencyGraph {
    edges: HashMap<PathBuf, Vec<PathBuf>>,
    src_dir: PathBuf,
}

impl DependencyGraph {
    /// Scan all `.rs` files under `src_dir` and build the dependency graph.
    ///
    /// Each file is read and its `use crate::` and `mod name;` statements are
    /// resolved to concrete paths within `src_dir`. Files that cannot be read
    /// are skipped silently; unresolvable imports produce no edges.
    pub fn from_src_dir(src_dir: &Path) -> anyhow::Result<Self> {
        let mut edges: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
        for file in collect_rs_files(src_dir) {
            let deps = scan_file_deps(&file, src_dir);
            edges.insert(file, deps);
        }
        Ok(Self {
            edges,
            src_dir: src_dir.to_owned(),
        })
    }

    /// Return the direct dependencies of `file`, or an empty slice if unknown.
    ///
    /// Called by tier assignment to walk the graph one step at a time.
    pub fn direct_deps(&self, file: &Path) -> &[PathBuf] {
        self.edges.get(file).map_or(&[], Vec::as_slice)
    }

    /// Return all files that `target` transitively depends on, including
    /// `target` itself. Circular references are handled by tracking visited
    /// nodes - each file appears at most once in the result.
    pub fn transitive_deps(&self, target: &Path) -> Vec<PathBuf> {
        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut ctx = DfsContext::builder()
            .edges(&self.edges)
            .visited(&mut visited)
            .out(&mut result)
            .build();
        collect_transitive(target, &mut ctx);
        result
    }

    /// Expose the `src_dir` this graph was built from.
    ///
    /// Used by the cache actor to verify that file paths are within scope.
    pub fn src_dir(&self) -> &Path {
        &self.src_dir
    }
}

/// Mutable traversal context for one transitive dependency walk.
#[derive(bon::Builder)]
struct DfsContext<'a> {
    edges: &'a HashMap<PathBuf, Vec<PathBuf>>,
    visited: &'a mut HashSet<PathBuf>,
    out: &'a mut Vec<PathBuf>,
}

/// Walk the dependency graph depth-first, collecting all reachable nodes.
///
/// `ctx.visited` guards against infinite loops caused by circular references.
/// Results are appended to `ctx.out` in DFS post-order (dependencies before
/// dependents), which naturally places roots at the front.
fn collect_transitive(node: &Path, ctx: &mut DfsContext<'_>) {
    if !ctx.visited.insert(node.to_owned()) {
        return;
    }
    if let Some(deps) = ctx.edges.get(node) {
        for dep in deps {
            collect_transitive(dep, ctx);
        }
    }
    ctx.out.push(node.to_owned());
}

/// Collect all `.rs` files under `dir` recursively.
///
/// Returns absolute `PathBuf`s. Non-UTF-8 filenames and unreadable dirs are
/// skipped. Called once per `DependencyGraph::from_src_dir`.
fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_recursive(dir, &mut files);
    files
}

fn collect_rs_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_recursive(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Parse one `.rs` file and return its intra-project dependencies.
///
/// Applies two patterns: `use crate::path::to::item;` and `mod name;`.
/// Unresolvable paths (files not present on disk) are skipped silently.
/// Called once per file by `DependencyGraph::from_src_dir`.
fn scan_file_deps(file: &Path, src_dir: &Path) -> Vec<PathBuf> {
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let mut deps = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if let Some(resolved) = try_resolve_use_crate(line, src_dir) {
            deps.push(resolved);
        }
        if let Some(resolved) = try_resolve_mod_decl(line, file, src_dir) {
            deps.push(resolved);
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

/// Try to resolve a `use crate::path::to::item;` line to a file path.
///
/// Strips the `use crate::` prefix and the final `::Item` segment (the item
/// name), then maps the remaining module path to `src_dir/path/to.rs` or
/// `src_dir/path/to/mod.rs`. Returns `None` if the line does not match or
/// neither candidate file exists.
fn try_resolve_use_crate(line: &str, src_dir: &Path) -> Option<PathBuf> {
    let rest = line.strip_prefix("use crate::")?;
    // Strip trailing `;`, optional `{...}` import group, or `as ...`
    let rest = rest.split(';').next()?.trim();
    let rest = rest.split(" as ").next()?.trim();
    // Convert `::` path to filesystem separators and drop the last segment
    let segments: Vec<&str> = rest.split("::").collect();
    if segments.is_empty() {
        return None;
    }
    // Try: treat all segments as module path (last may be a module, not type)
    let candidate_full = build_candidate(src_dir, &segments);
    if let Some(p) = candidate_full {
        return Some(p);
    }
    // Drop the last segment (it is the item name) and resolve the module
    if segments.len() >= 2 {
        let module_segs = &segments[..segments.len() - 1];
        build_candidate(src_dir, module_segs)
    } else {
        None
    }
}

/// Build a candidate path from module segments, trying `seg.rs` then `seg/mod.rs`.
///
/// `segments` maps directly to filesystem path components under `src_dir`.
/// Returns `Some(path)` if one of the candidates exists on disk.
fn build_candidate(src_dir: &Path, segments: &[&str]) -> Option<PathBuf> {
    if segments.is_empty() {
        return None;
    }
    let mut base = src_dir.to_owned();
    for &seg in &segments[..segments.len() - 1] {
        base.push(seg);
    }
    let last = segments[segments.len() - 1];
    let rs = base.join(format!("{last}.rs"));
    if rs.exists() {
        return Some(rs);
    }
    let mod_rs = base.join(last).join("mod.rs");
    if mod_rs.exists() {
        return Some(mod_rs);
    }
    None
}

/// Try to resolve a `mod name;` declaration to a sibling file path.
///
/// Only bare `mod name;` declarations are matched (no `pub(crate)` etc. needed;
/// any leading visibility is fine as long as `mod ` and `;` are present).
/// Sibling is resolved relative to the declaring file's directory.
/// Returns `None` if the line does not match or candidate file does not exist.
fn try_resolve_mod_decl(line: &str, declaring_file: &Path, _src_dir: &Path) -> Option<PathBuf> {
    // Match: (optional visibility) `mod name ;`
    let rest = line
        .strip_prefix("pub(crate) mod ")
        .or_else(|| line.strip_prefix("pub mod "))
        .or_else(|| line.strip_prefix("mod "))
        .or_else(|| line.strip_prefix("pub(super) mod "))?;
    let name = rest.strip_suffix(';')?.trim();
    if name.is_empty() || name.contains(' ') {
        return None;
    }
    let parent = declaring_file.parent()?;
    // Try sibling `name.rs`
    let sibling_rs = parent.join(format!("{name}.rs"));
    if sibling_rs.exists() {
        return Some(sibling_rs);
    }
    // Try `name/mod.rs`
    let sibling_mod = parent.join(name).join("mod.rs");
    if sibling_mod.exists() {
        return Some(sibling_mod);
    }
    None
}
