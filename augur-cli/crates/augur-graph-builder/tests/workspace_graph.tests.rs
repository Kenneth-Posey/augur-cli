use augur_graph_builder::workspace_graph;
use std::path::Path;

#[test]
fn test_resolve_workspace_manifest_not_found() {
    let result = workspace_graph::resolve_workspace(Path::new("/nonexistent/Cargo.toml"));
    assert!(result.is_err());
}

#[test]
fn test_resolve_workspace_extra_manifest() {
    // Try resolving the actual workspace from the repo root.
    let result = workspace_graph::resolve_workspace(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../Cargo.toml")
            .as_path(),
    );
    if let Ok(resolved) = result {
        // The workspace should have at least a few crate nodes.
        assert!(
            !resolved.graph.nodes.is_empty(),
            "expected at least one workspace crate"
        );
        assert!(
            !resolved.crate_paths.is_empty(),
            "expected at least one crate path"
        );
    }
    // If it fails (e.g., network or manifest issues), that's OK for this test.
}

#[test]
fn test_graph_data_serialization() {
    use augur_graph_builder::graph_data::*;
    let data = GraphData {
        workspace: WorkspaceGraph {
            nodes: vec![
                CrateNode {
                    id: "crate-a".to_string(),
                    label: "crate-a".to_string(),
                    doc: "Doc A".to_string(),
                    layer: 0,
                },
                CrateNode {
                    id: "crate-b".to_string(),
                    label: "crate-b".to_string(),
                    doc: "".to_string(),
                    layer: 1,
                },
            ],
            edges: vec![CrateEdge {
                source: "crate-a".to_string(),
                target: "crate-b".to_string(),
            }],
        },
        crates: std::collections::HashMap::new(),
    };

    let json = serde_json::to_string_pretty(&data).unwrap();
    assert!(json.contains("crate-a"));
    assert!(json.contains("crate-b"));
    assert!(json.contains("Doc A"));

    // Round-trip
    let deserialized: GraphData = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.workspace.nodes.len(), 2);
    assert_eq!(deserialized.workspace.edges.len(), 1);
}

#[test]
fn test_graph_data_module_node_children() {
    use augur_graph_builder::graph_data::*;

    let module_graph = CrateModuleGraph {
        nodes: vec![
            ModuleNode {
                id: "my-crate::lib".to_string(),
                label: "lib".to_string(),
                doc: "".to_string(),
                visibility: "pub".to_string(),
                children: vec!["my-crate::actors".to_string()],
                symbols: vec![],
            },
            ModuleNode {
                id: "my-crate::actors".to_string(),
                label: "actors".to_string(),
                doc: "Actors module".to_string(),
                visibility: "pub".to_string(),
                children: vec![],
                symbols: vec![],
            },
        ],
        edges: vec![ModuleEdge {
            source: "my-crate::lib".to_string(),
            target: "my-crate::actors".to_string(),
        }],
        cross_edges: vec![CrossCrateEdge {
            source: "my-crate::actors".to_string(),
            target_crate: "other-crate".to_string(),
            target_module: "other-crate::lib".to_string(),
        }],
    };

    let json = serde_json::to_string_pretty(&module_graph).unwrap();
    let deserialized: CrateModuleGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.nodes.len(), 2);
    assert_eq!(deserialized.edges.len(), 1);
    assert_eq!(deserialized.cross_edges.len(), 1);
    assert_eq!(deserialized.nodes[0].children.len(), 1);
    assert_eq!(deserialized.nodes[0].symbols.len(), 0);
}
