//! Meta-planning pure functions used by the supervisor to generate plan trees.
//!
//! `build_meta_prompt` constructs the system/user prompt sent to the executor
//! that instructs it to decompose a high-level goal into an ordered sequence of
//! `update_plan_step` tool calls. The actor shell drains executor output and
//! applies plan-node updates using the helpers in this module.

use augur_domain::domain::plan_tree::{NodeStatus, PlanNode, PlanNodeId, PlanTree};
use augur_domain::domain::string_newtypes::{GoalText, OutputText, PromptText, StringNewtype};
use augur_domain::domain::types::AgentOutput;

// ── MetaPlanError ─────────────────────────────────────────────────────────────

/// Errors returned by `run_meta_plan`.
#[derive(Debug)]
pub enum MetaPlanError {
    /// The broadcast channel was closed before `TurnComplete` was received.
    ChannelClosed,
}

/// Progress signal for a single meta-planning output event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetaTurnProgress {
    Complete,
    Continue,
}

impl std::fmt::Display for MetaPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChannelClosed => write!(f, "executor output channel closed before TurnComplete"),
        }
    }
}

// ── build_meta_prompt ─────────────────────────────────────────────────────────

/// Constructs the system-level meta-planning prompt for a given goal.
///
/// Call context: called by `handle_start_plan` before invoking
/// `executor.send_prompt`. The resulting string is sent verbatim to the
/// executor in `ExecutorMode::Plan`.
///
/// Returns `PromptText` containing the full prompt with the goal embedded and
/// a reference to `update_plan_step` so the executor knows which tool to use.
pub fn build_meta_prompt(goal: &GoalText) -> PromptText {
    PromptText::new(format!(
        "You are a plan decomposition engine.\n\
         \n\
         Your job is to break down the following goal into a sequence of\n\
         concrete, atomic implementation steps. For each step, call the\n\
         `update_plan_step` tool with the step id, title, and optional\n\
         step-file path. Steps must be leaf-level actions (e.g.,\n\
         \"add field X to struct Y in src/foo.rs\").\n\
         \n\
         Goal:\n\
         {goal}\n\
         \n\
         When you have emitted all steps, stop. Do not emit prose or\n\
         explanations - only `update_plan_step` tool calls."
    ))
}

// ── apply_meta_output ────────────────────────────────────────────────────────

/// Apply a single executor output event to the in-progress meta-plan tree.
///
/// Returns `true` when the event completes the meta-planning turn.
pub(crate) fn apply_meta_output(tree: &mut PlanTree, output: AgentOutput) -> MetaTurnProgress {
    match output {
        AgentOutput::TurnComplete => MetaTurnProgress::Complete,
        AgentOutput::PlanNodeUpdate {
            node_id,
            status,
            notes,
        } => {
            let params = PlanNodeUpdateParams::builder()
                .node_id(node_id)
                .status(status)
                .maybe_notes(notes)
                .build();
            apply_plan_node_update(tree, params);
            MetaTurnProgress::Continue
        }
        _ => MetaTurnProgress::Continue,
    }
}

/// Parameters for updating a plan node.
///
/// Bundles the node identity, desired status, and optional notes into a single
/// value so that `apply_plan_node_update` stays within the three-parameter limit.
#[derive(Debug, Clone, bon::Builder)]
pub struct PlanNodeUpdateParams {
    /// Node ID to update.
    pub node_id: PlanNodeId,
    /// New status for the node.
    pub status: NodeStatus,
    /// Optional output/notes attached to the update.
    #[builder(into)]
    pub notes: Option<OutputText>,
}

/// Applies a `PlanNodeUpdateParams` to `tree`.
///
/// If the node already exists in the tree its status is updated in-place.
/// When the node is not found a new leaf is appended using the notes text (or
/// the node id) as the title.
fn apply_plan_node_update(tree: &mut PlanTree, params: PlanNodeUpdateParams) {
    let PlanNodeUpdateParams {
        node_id,
        status,
        notes,
    } = params;
    let found = tree.update_node_status(&node_id, status);
    if found.is_some() {
        return;
    }
    let title = notes.unwrap_or_else(|| OutputText::new(node_id.to_string()));
    let step_file = format!("steps/{node_id}.md");
    tree.root
        .children
        .push(PlanNode::new_leaf(node_id, title.into_inner(), step_file));
}
