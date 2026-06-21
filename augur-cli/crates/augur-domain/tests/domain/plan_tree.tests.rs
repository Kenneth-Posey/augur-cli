#![allow(clippy::duplicate_mod)]
use augur_domain::domain::plan_tree::{
    CheckpointConfig, NodeKind, NodeStatus, PlanNode, PlanNodeId, PlanTree, PlanTreeId,
};
use augur_domain::domain::string_newtypes::StringNewtype;

#[path = "../support/rustdoc.tests.rs"]
mod rustdoc_support;

// ── PlanNode construction ──────────────────────────────────────────────────

/// Verifies that new_leaf creates a node with Pending status, Leaf kind,
/// and the given step_file set on NodeConfig.
#[test]
fn plan_node_new_leaf_has_pending_status() {
    let node = PlanNode::new_leaf("n1", "Install deps", "steps/n1.md");
    assert_eq!(node.status, NodeStatus::Pending);
    assert_eq!(node.config.kind, NodeKind::Leaf);
    assert_eq!(node.config.step_file.as_deref(), Some("steps/n1.md"));
    assert!(node.children.is_empty());
}

/// Verifies that new_branch creates a node with Pending status, Branch kind,
/// no step_file, and no children.
#[test]
fn plan_node_new_branch_has_no_children_and_branch_kind() {
    let node = PlanNode::new_branch("b1", "Setup phase");
    assert_eq!(node.status, NodeStatus::Pending);
    assert_eq!(node.config.kind, NodeKind::Branch);
    assert!(node.config.step_file.is_none());
    assert!(node.children.is_empty());
}

/// Verifies that with_checkpoint attaches a CheckpointConfig to a node.
#[test]
fn plan_node_with_checkpoint_sets_config() {
    let node = PlanNode::new_branch("b1", "Phase boundary").with_checkpoint(CheckpointConfig {
        commit: true.into(),
        compact: false.into(),
    });
    assert!(node.config.checkpoint.is_some());
    let cp = node.config.checkpoint.unwrap();
    assert!(cp.commit.0);
    assert!(!cp.compact.0);
}

/// Verifies that add_child appends the child to the node's children list.
#[test]
fn plan_node_add_child_appends_child() {
    let leaf = PlanNode::new_leaf("l1", "Leaf", "steps/l1.md");
    let branch = PlanNode::new_branch("b1", "Branch").add_child(leaf);
    assert_eq!(branch.children.len(), 1);
    assert_eq!(branch.children[0].id, PlanNodeId::new("l1"));
}

// ── PlanNode::find_mut ────────────────────────────────────────────────────

/// Verifies that find_mut on a node returns itself when the id matches.
#[test]
fn plan_node_find_mut_returns_self() {
    let mut node = PlanNode::new_branch("b1", "Root");
    let found = node.find_mut(&PlanNodeId::new("b1"));
    assert!(found.is_some());
}

/// Verifies that find_mut locates a nested node by id using depth-first search.
#[test]
fn plan_node_find_mut_locates_nested_node_by_id() {
    let leaf = PlanNode::new_leaf("l1", "Leaf", "steps/l1.md");
    let mut branch = PlanNode::new_branch("b1", "Branch").add_child(leaf);
    let found = branch.find_mut(&PlanNodeId::new("l1"));
    assert!(found.is_some());
    found.unwrap().status = NodeStatus::Done;
    assert_eq!(branch.children[0].status, NodeStatus::Done);
}

/// Verifies that find_mut returns None when no node has the given id.
#[test]
fn plan_node_find_mut_returns_none_for_unknown_id() {
    let mut node = PlanNode::new_branch("b1", "Branch");
    let found = node.find_mut(&PlanNodeId::new("missing"));
    assert!(found.is_none());
}

// ── PlanNode::next_pending_leaf ───────────────────────────────────────────

/// Verifies that next_pending_leaf returns the first Pending Leaf node found
/// in depth-first order.
#[test]
fn plan_node_next_pending_leaf_returns_first_pending() {
    let l1 = PlanNode::new_leaf("l1", "Step 1", "steps/l1.md");
    let l2 = PlanNode::new_leaf("l2", "Step 2", "steps/l2.md");
    let branch = PlanNode::new_branch("b1", "Branch")
        .add_child(l1)
        .add_child(l2);
    let next = branch.next_pending_leaf();
    assert!(next.is_some());
    assert_eq!(next.unwrap().id, PlanNodeId::new("l1"));
}

/// Verifies that next_pending_leaf skips nodes with Done status.
#[test]
fn plan_node_next_pending_leaf_skips_done_nodes() {
    let mut l1 = PlanNode::new_leaf("l1", "Done step", "steps/l1.md");
    l1.status = NodeStatus::Done;
    let l2 = PlanNode::new_leaf("l2", "Pending step", "steps/l2.md");
    let branch = PlanNode::new_branch("b1", "Branch")
        .add_child(l1)
        .add_child(l2);
    let next = branch.next_pending_leaf();
    assert_eq!(next.unwrap().id, PlanNodeId::new("l2"));
}

/// Verifies that next_pending_leaf returns None when all leaf nodes are Done.
#[test]
fn plan_node_next_pending_leaf_returns_none_when_all_done() {
    let mut l1 = PlanNode::new_leaf("l1", "Step", "steps/l1.md");
    l1.status = NodeStatus::Done;
    let branch = PlanNode::new_branch("b1", "Branch").add_child(l1);
    assert!(branch.next_pending_leaf().is_none());
}

/// Verifies that next_pending_leaf returns None for a branch node with no children.
#[test]
fn plan_node_next_pending_leaf_empty_branch_returns_none() {
    let branch = PlanNode::new_branch("b1", "Empty branch");
    assert!(branch.next_pending_leaf().is_none());
}

// ── PlanTree ──────────────────────────────────────────────────────────────

/// Verifies that PlanTree::new creates a tree whose root is a Branch node
/// with the same id as the tree, and an empty children list.
#[test]
fn plan_tree_new_creates_branch_root_with_tree_id() {
    let tree = PlanTree::new("t1", "My Plan", "Add a feature");
    assert_eq!(tree.id, PlanTreeId::new("t1"));
    assert_eq!(tree.root.config.kind, NodeKind::Branch);
    assert_eq!(tree.root.id, PlanNodeId::new("t1"));
    assert!(tree.root.children.is_empty());
}

/// Verifies that update_node_status returns Some(()) and mutates the node when
/// the id exists in the tree.
#[test]
fn plan_tree_update_node_status_returns_true_on_found() {
    let leaf = PlanNode::new_leaf("l1", "Step", "steps/l1.md");
    let mut tree = PlanTree::new("t1", "Plan", "goal");
    tree.root = tree.root.add_child(leaf);
    let changed = tree.update_node_status(&PlanNodeId::new("l1"), NodeStatus::Done);
    assert_eq!(changed, Some(()));
}

/// Verifies that update_node_status returns None when the id is not in the tree.
#[test]
fn plan_tree_update_node_status_returns_false_on_missing_id() {
    let mut tree = PlanTree::new("t1", "Plan", "goal");
    let changed = tree.update_node_status(&PlanNodeId::new("missing"), NodeStatus::Done);
    assert_eq!(changed, None);
}

/// Verifies that update_node_status correctly applies a Failed status with a message.
#[test]
fn plan_tree_update_node_status_applies_failed_variant() {
    let leaf = PlanNode::new_leaf("l1", "Step", "steps/l1.md");
    let mut tree = PlanTree::new("t1", "Plan", "goal");
    tree.root = tree.root.add_child(leaf);
    tree.update_node_status(
        &PlanNodeId::new("l1"),
        NodeStatus::Failed("build error".into()),
    );
    let node = tree.root.find_mut(&PlanNodeId::new("l1")).unwrap();
    assert!(matches!(node.status, NodeStatus::Failed(_)));
}

/// Verifies that next_pending_leaf on the tree delegates to the root node.
#[test]
fn plan_tree_next_pending_leaf_delegates_to_root() {
    let leaf = PlanNode::new_leaf("l1", "Step", "steps/l1.md");
    let mut tree = PlanTree::new("t1", "Plan", "goal");
    tree.root = tree.root.add_child(leaf);
    let next = tree.next_pending_leaf();
    assert_eq!(next.unwrap().id, PlanNodeId::new("l1"));
}

// ── Serde round-trip ─────────────────────────────────────────────────────

/// Verifies that PlanTree serializes to JSON and deserializes back to an
/// equal value (all fields preserved).
#[test]
fn plan_tree_serde_round_trip() {
    let leaf = PlanNode::new_leaf("l1", "Step", "steps/l1.md").with_checkpoint(CheckpointConfig {
        commit: true.into(),
        compact: true.into(),
    });
    let mut tree = PlanTree::new("t1", "Plan", "goal");
    tree.root = tree.root.add_child(leaf);

    let json = serde_json::to_string(&tree).expect("serialize");
    let restored: PlanTree = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(tree.id, restored.id);
    assert_eq!(tree.root.children[0].id, restored.root.children[0].id);
    assert_eq!(
        tree.root.children[0]
            .config
            .checkpoint
            .as_ref()
            .unwrap()
            .commit,
        restored.root.children[0]
            .config
            .checkpoint
            .as_ref()
            .unwrap()
            .commit,
    );
}

/// Verifies that PLAN_STEP_FILE_EXT is ".md", matching the step file
/// extension used by PlanNode::new_leaf and PlanTreeStore::write_step.
#[test]
fn plan_step_file_ext_is_dot_md() {
    use augur_domain::domain::plan_tree::PLAN_STEP_FILE_EXT;
    assert_eq!(PLAN_STEP_FILE_EXT, ".md");
}

/// Verifies Phase 1 plan-tree APIs use FilePath and Option<()> in public signatures.
#[test]
fn plan_tree_phase_one_public_api_uses_domain_wrappers() {
    let plan_node_html =
        rustdoc_support::rustdoc_html("augur_domain/domain/plan_tree/struct.PlanNode.html");
    assert!(
        plan_node_html.contains("struct.FilePath.html"),
        "expected PlanNode rustdoc to reference FilePath for step_file",
    );

    let plan_tree_html =
        rustdoc_support::rustdoc_html("augur_domain/domain/plan_tree/struct.PlanTree.html");
    assert!(
        plan_tree_html.contains("Option&lt;()&gt;"),
        "expected PlanTree::update_node_status rustdoc to return Option<()>",
    );
}
