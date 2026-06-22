//! Tests for dependency graph parsing from intra-project Rust source files.

use augur_core::actors::cache::deps::DependencyGraph;
use std::fs;
use tempfile::TempDir;

/// Creates a temp project structure with:
/// - src/main.rs: `use crate::domain::types::Foo;`
/// - src/domain/mod.rs: `pub mod types;`
/// - src/domain/types.rs: (no deps)
fn make_temp_project() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(src.join("domain")).unwrap();
    fs::write(
        src.join("main.rs"),
        "use crate::domain::types::Foo;\nfn main() {}\n",
    )
    .unwrap();
    fs::write(src.join("domain").join("mod.rs"), "pub mod types;\n").unwrap();
    fs::write(src.join("domain").join("types.rs"), "pub struct Foo;\n").unwrap();
    dir
}

/// `use crate::domain::types::Foo;` in main.rs resolves to a dep on
/// `src/domain/types.rs`.
#[test]
fn dep_graph_resolves_use_crate_import() {
    let dir = make_temp_project();
    let src = dir.path().join("src");
    let graph = DependencyGraph::from_src_dir(&src).unwrap();
    let main = src.join("main.rs");
    let types = src.join("domain").join("types.rs");
    let deps = graph.direct_deps(&main);
    assert!(
        deps.contains(&types),
        "main.rs should depend on domain/types.rs, got: {deps:?}"
    );
}

/// `pub mod types;` in domain/mod.rs resolves to a dep on `domain/types.rs`.
#[test]
fn dep_graph_resolves_mod_declaration() {
    let dir = make_temp_project();
    let src = dir.path().join("src");
    let graph = DependencyGraph::from_src_dir(&src).unwrap();
    let mod_file = src.join("domain").join("mod.rs");
    let types = src.join("domain").join("types.rs");
    let deps = graph.direct_deps(&mod_file);
    assert!(
        deps.contains(&types),
        "domain/mod.rs should depend on domain/types.rs, got: {deps:?}"
    );
}

/// An import that points to a file outside the project resolves to nothing.
#[test]
fn dep_graph_skips_unresolvable_imports() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("main.rs"),
        "use crate::nonexistent::Thing;\nfn main() {}\n",
    )
    .unwrap();
    let graph = DependencyGraph::from_src_dir(&src).unwrap();
    let main = src.join("main.rs");
    let deps = graph.direct_deps(&main);
    assert!(
        deps.is_empty(),
        "unresolvable import should produce no deps, got: {deps:?}"
    );
}

/// `transitive_deps` for main.rs includes domain/types.rs even though
/// main.rs only directly depends on domain/types.rs (one hop).
#[test]
fn transitive_deps_includes_indirect_deps() {
    let dir = make_temp_project();
    let src = dir.path().join("src");
    let graph = DependencyGraph::from_src_dir(&src).unwrap();
    let main = src.join("main.rs");
    let types = src.join("domain").join("types.rs");
    let transitive = graph.transitive_deps(&main);
    assert!(
        transitive.contains(&types),
        "transitive deps of main.rs should include domain/types.rs, got: {transitive:?}"
    );
}

/// `transitive_deps` terminates without infinite loop when A depends on B
/// and B depends on A (circular reference).
#[test]
fn transitive_deps_handles_circular_refs() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    // A uses crate::b::B
    fs::write(src.join("a.rs"), "use crate::b::B;\npub struct A;\n").unwrap();
    // B uses crate::a::A  (circular)
    fs::write(src.join("b.rs"), "use crate::a::A;\npub struct B;\n").unwrap();
    let graph = DependencyGraph::from_src_dir(&src).unwrap();
    let a = src.join("a.rs");
    let b = src.join("b.rs");
    let transitive = graph.transitive_deps(&a);
    // Both nodes should appear exactly once each (no infinite loop).
    assert!(transitive.contains(&b), "transitive should include b.rs");
    let b_count = transitive.iter().filter(|p| *p == &b).count();
    assert_eq!(b_count, 1, "b.rs should appear exactly once");
}
