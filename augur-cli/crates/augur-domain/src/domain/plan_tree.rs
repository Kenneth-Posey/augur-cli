//! Plan tree domain types: in-memory recursive tree, node status, serialization.
//!
//! All types here are pure data - no I/O, no async. The disk store lives in
//! `src/plan_store/mod.rs` following the same pattern as `src/persistence/`.

use serde::{Deserialize, Serialize};

use crate::domain::newtypes::IsPredicate;
use crate::domain::string_newtypes::{FailureReason, GoalText, OutputText, PlanName};
pub use crate::domain::string_newtypes::{FilePath, PlanNodeId, PlanTreeId, StringNewtype};

/// File extension for plan step documents stored on disk.
///
/// Step files are Markdown documents placed under `{plan_dir}/steps/`.
/// Consumers: `PlanNode::new_leaf` (step_file field), `PlanTreeStore::write_step`,
/// `SupervisorActor::begin_execution` (file reads).
pub const PLAN_STEP_FILE_EXT: &str = ".md";

/// Execution status of a plan node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum NodeStatus {
    /// Node has not been started.
    Pending,
    /// Node is actively being executed.
    InProgress,
    /// Node completed successfully.
    Done,
    /// Node failed; inner string carries the failure reason.
    Failed(FailureReason),
}

/// Structural role of a node in the plan tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Contains child nodes; never executed directly.
    Branch,
    /// Atomic executable step; has an associated step file.
    Leaf,
}

/// Controls whether a checkpoint fires after this node completes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointConfig {
    /// Trigger a git commit after this node completes.
    pub commit: IsPredicate,
    /// Trigger a conversation compact after this node completes.
    pub compact: IsPredicate,
}

/// Non-lifecycle configuration grouped onto a node.
///
/// Extracted as a sub-struct so `PlanNode` stays within the 5-field limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Whether this is a branch or a leaf node.
    pub kind: NodeKind,
    /// Optional checkpoint to fire after this node completes.
    pub checkpoint: Option<CheckpointConfig>,
    /// Relative path to the step file, e.g. `"steps/{id}.md"`.
    /// Only set for `Leaf` nodes.
    pub step_file: Option<FilePath>,
    /// Executor-set notes, typically a failure reason or summary.
    pub notes: Option<OutputText>,
}

/// A single node in the plan tree.
///
/// Branch nodes group leaf children; leaf nodes carry an executable step file.
/// The 5-field limit is satisfied via the `NodeConfig` sub-struct.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanNode {
    /// Unique identifier for this node within the tree.
    pub id: PlanNodeId,
    /// Human-readable description of the work.
    pub title: PlanName,
    /// Current execution status.
    pub status: NodeStatus,
    /// Non-lifecycle configuration (kind, checkpoint, step file, notes).
    pub config: NodeConfig,
    /// Child nodes; empty for leaf nodes.
    pub children: Vec<PlanNode>,
}

/// The complete in-memory plan tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTree {
    /// Unique identifier for the plan (used as the directory name on disk).
    pub id: PlanTreeId,
    /// Human-readable plan title.
    pub title: PlanName,
    /// The high-level goal that was used to generate this plan.
    pub goal: GoalText,
    /// Root node of the tree; always a `Branch`.
    pub root: PlanNode,
}

impl PlanNode {
    /// Creates a new leaf node with `Pending` status.
    ///
    /// Use for atomic executable steps. The `step_file` path is relative to
    /// the plan directory, e.g. `"steps/{id}.md"`.
    pub fn new_leaf(
        id: impl Into<PlanNodeId>,
        title: impl Into<PlanName>,
        step_file: impl Into<FilePath>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: NodeStatus::Pending,
            config: NodeConfig {
                kind: NodeKind::Leaf,
                checkpoint: None,
                step_file: Some(step_file.into()),
                notes: None,
            },
            children: Vec::new(),
        }
    }

    /// Creates a new branch node with `Pending` status and no children.
    ///
    /// Use for grouping leaf nodes. Branch nodes are never executed directly.
    pub fn new_branch(id: impl Into<PlanNodeId>, title: impl Into<PlanName>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: NodeStatus::Pending,
            config: NodeConfig {
                kind: NodeKind::Branch,
                checkpoint: None,
                step_file: None,
                notes: None,
            },
            children: Vec::new(),
        }
    }

    /// Attaches a `CheckpointConfig` to this node, returning the modified node.
    ///
    /// The checkpoint fires after the node (or all its descendants) complete.
    pub fn with_checkpoint(mut self, config: CheckpointConfig) -> Self {
        self.config.checkpoint = Some(config);
        self
    }

    /// Appends `child` to this node's children list, returning the modified node.
    pub fn add_child(mut self, child: PlanNode) -> Self {
        self.children.push(child);
        self
    }

    /// Returns a mutable reference to the node with `id` using depth-first search.
    ///
    /// Returns `None` if no matching node exists in the subtree.
    pub fn find_mut(&mut self, id: &PlanNodeId) -> Option<&mut PlanNode> {
        if self.id == *id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }
        None
    }

    /// Returns a reference to the first `Pending` `Leaf` node in depth-first order.
    ///
    /// Returns `None` when all leaf nodes are done or when the subtree has no leaves.
    pub fn next_pending_leaf(&self) -> Option<&PlanNode> {
        if self.config.kind == NodeKind::Leaf && self.status == NodeStatus::Pending {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.next_pending_leaf() {
                return Some(found);
            }
        }
        None
    }
}

impl PlanTree {
    /// Creates a new plan tree with a root branch node whose id matches the tree id.
    ///
    /// The root is always a `Branch` - leaf nodes are children of the root or
    /// of intermediate branch nodes.
    pub fn new(
        id: impl Into<PlanTreeId>,
        title: impl Into<PlanName>,
        goal: impl Into<GoalText>,
    ) -> Self {
        let tree_id: PlanTreeId = id.into();
        let root_id = PlanNodeId::new(tree_id.as_str());
        Self {
            id: tree_id,
            title: title.into(),
            goal: goal.into(),
            root: PlanNode::new_branch(root_id, "root"),
        }
    }

    /// Updates the status of the node with the given `id` in the tree.
    ///
    /// Returns `Option<()>`: `Some(())` if the node was found and updated, otherwise `None`.
    pub fn update_node_status(&mut self, id: &PlanNodeId, status: NodeStatus) -> Option<()> {
        self.root.find_mut(id).map(|node| {
            node.status = status;
        })
    }

    /// Returns the first `Pending` `Leaf` node in the tree using depth-first order.
    ///
    /// Delegates to `root.next_pending_leaf()`. Returns `None` when the plan is
    /// fully executed or has no leaves.
    pub fn next_pending_leaf(&self) -> Option<&PlanNode> {
        self.root.next_pending_leaf()
    }
}
