//! `SupervisorActor` - orchestrates plan tree execution via an `ExecutorDriver`.
//!
//! Spawns a tokio task that handles `SupervisorCmd` messages. Walks the plan
//! tree depth-first, dispatching each leaf step to the executor. After each
//! step, `evaluate_gate` decides pass or fail. Checkpoints fire when the
//! `CheckpointTracker` or the node's `CheckpointConfig` triggers.

use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use crate::plan_store::PlanTreeStore;
use augur_domain::domain::channels::SUPERVISOR_COMMAND_CAPACITY;
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::plan_tree::{
    NodeStatus, PlanNode, PlanNodeId, PlanTree, PlanTreeId, StringNewtype,
};
use augur_domain::domain::string_newtypes::{
    FailureReason, GoalText, OutputText, PromptText, StepFileName,
};
use augur_domain::domain::traits::{ExecutorDriver, ExecutorMode};
use augur_domain::domain::types::{AgentOutput, SupervisorEvent};

use super::checkpoint::CheckpointTracker;
use super::commands::SupervisorCmd;
use super::handle::{make_event_channel, SupervisorHandle};
use super::meta_planner::{apply_meta_output, build_meta_prompt, MetaPlanError};
use super::phase_gate::{evaluate_gate, StepOutcome};

// ── LeafInfo ──────────────────────────────────────────────────────────────────

/// Data extracted from a pending leaf before releasing the `Arc<PlanTree>` borrow.
///
/// Carries the fields needed for the executor step without holding a reference
/// into the tree - avoids borrow conflicts when we later call `Arc::make_mut`.
struct LeafInfo {
    /// Full clone of the pending leaf node.
    node: PlanNode,
    /// The plan tree id, used for step-file reads.
    plan_id: PlanTreeId,
}

// ── SupervisorState ───────────────────────────────────────────────────────────

/// Owned state of a running `SupervisorActor`.
///
/// Exactly 5 fields per the struct decomposition rule. `running` is a local
/// variable in `run()` so it does not count toward the field limit.
#[derive(bon::Builder)]
struct SupervisorState {
    /// CLI session driver; injected by `wiring.rs` via `Box<dyn ExecutorDriver>`.
    executor: Box<dyn ExecutorDriver + Send + Sync>,
    /// Disk store for saving/loading plan trees and reading step files.
    store: PlanTreeStore,
    /// Heuristic tracker for automatic checkpoint firing.
    checkpoint: CheckpointTracker,
    /// The currently active plan; `None` when idle.
    active_plan: Option<Arc<PlanTree>>,
    /// Broadcast sender for supervisor events; shared with the handle.
    event_tx: broadcast::Sender<SupervisorEvent>,
}

// ── SupervisorActor ───────────────────────────────────────────────────────────

/// Spawns the supervisor task and returns a cloneable `SupervisorHandle`.
///
/// Call context: called once from `wiring.rs` during startup. `executor` must
/// be a live `ExecutorHandle` (or any `ExecutorDriver` impl). `store_dir` is
/// the base directory for persisted plan trees.
pub struct SupervisorActor;

impl SupervisorActor {
    /// Spawn the supervisor task and return a `SupervisorHandle`.
    ///
    /// The returned handle is cloneable and can be passed to the TUI and other
    /// actors that need to start plans or subscribe to events.
    pub fn spawn(
        executor: Box<dyn ExecutorDriver + Send + Sync>,
        store: PlanTreeStore,
    ) -> SupervisorHandle {
        let event_tx = make_event_channel();
        let (cmd_tx, cmd_rx) = mpsc::channel::<SupervisorCmd>(*SUPERVISOR_COMMAND_CAPACITY);
        let handle = SupervisorHandle::new(cmd_tx, event_tx.clone());
        let state = SupervisorState::builder()
            .executor(executor)
            .store(store)
            .checkpoint(CheckpointTracker::default())
            .event_tx(event_tx)
            .build();
        tokio::spawn(run(state, cmd_rx));
        handle
    }
}

// ── run ───────────────────────────────────────────────────────────────────────

/// Main event loop for the supervisor task.
async fn run(mut state: SupervisorState, mut cmd_rx: mpsc::Receiver<SupervisorCmd>) {
    info!("SupervisorActor started");
    let mut running = true;
    loop {
        let Some(cmd) = cmd_rx.recv().await else {
            break;
        };
        if handle_supervisor_command(&mut state, cmd, &mut running).await {
            break;
        }
    }
    info!("SupervisorActor stopped");
}

async fn handle_supervisor_command(
    state: &mut SupervisorState,
    cmd: SupervisorCmd,
    running: &mut bool,
) -> bool {
    if let SupervisorCmd::StartPlan { goal } = cmd {
        return handle_start_plan_command(state, goal, *running).await;
    }
    handle_non_start_supervisor_command(state, cmd, running).await
}

// ── handle_start_plan ─────────────────────────────────────────────────────────

/// Handles `SupervisorCmd::StartPlan`: builds tree via meta-planning, then walks it.
async fn handle_start_plan(state: &mut SupervisorState, goal: GoalText, running: bool) {
    let plan_id = PlanTreeId::new(uuid::Uuid::new_v4().to_string());
    debug!(plan_id = %plan_id, "SupervisorActor: starting plan");

    let tree_title = goal.clone().into_inner();
    let tree_goal = goal.into_inner();
    let mut tree = PlanTree::new(plan_id, tree_title, tree_goal);
    state.active_plan = Some(Arc::new(tree.clone()));

    let mut output_rx = state.executor.subscribe_output();
    let Some(active) = state.active_plan.as_ref() else {
        tracing::warn!("supervisor: expected active_plan but found None");
        return;
    };
    let prompt = build_meta_prompt(&active.goal);
    state.executor.set_mode(ExecutorMode::Plan).await;
    state.executor.send_prompt(prompt).await;

    if let Err(e) = run_meta_plan(&mut tree, &mut output_rx).await {
        warn!(error = %e, "SupervisorActor: meta-plan drain failed");
        emit(
            &state.event_tx,
            SupervisorEvent::Failed {
                reason: OutputText::new(e.to_string()),
            },
        );
        state.active_plan = None;
        return;
    }

    let tree_arc = Arc::new(tree);
    state.active_plan = Some(tree_arc.clone());

    if let Err(e) = state.store.save(&tree_arc).await {
        warn!(error = %e, "SupervisorActor: failed to save initial plan tree");
    }

    emit(&state.event_tx, SupervisorEvent::PlanGenerated(tree_arc));

    begin_execution(state, &mut output_rx, running).await;
}

// ── handle_cancel_plan ────────────────────────────────────────────────────────

/// Handles `SupervisorCmd::CancelPlan`: clears active plan and emits Failed.
async fn handle_cancel_plan(state: &mut SupervisorState) {
    if state.active_plan.take().is_some() {
        info!("SupervisorActor: plan cancelled");
        emit(
            &state.event_tx,
            SupervisorEvent::Failed {
                reason: OutputText::new("cancelled"),
            },
        );
    } else {
        debug!("SupervisorActor: CancelPlan received with no active plan");
    }
}

// ── handle_inject_step ────────────────────────────────────────────────────────

/// Handles `SupervisorCmd::InjectStep`: adds `node` as a child of `parent_id`.
fn handle_inject_step(state: &mut SupervisorState, parent_id: PlanNodeId, node: PlanNode) {
    let Some(arc) = state.active_plan.as_mut() else {
        warn!("InjectStep received with no active plan - ignoring");
        return;
    };
    let tree = Arc::make_mut(arc);
    match tree.root.find_mut(&parent_id) {
        Some(parent) => {
            parent.children.push(node);
            debug!(parent_id = %parent_id, "SupervisorActor: step injected");
        }
        None => {
            warn!(parent_id = %parent_id, "InjectStep: parent node not found - ignoring");
        }
    }
}

// ── begin_execution ───────────────────────────────────────────────────────────

/// Walks the plan tree, dispatching each pending leaf to the executor.
///
/// Checks `running` at the start of each iteration. When `running` is false,
/// execution halts immediately without emitting `ExecutionComplete`. This
/// allows a pre-flight `Pause` command to prevent execution from starting.
async fn begin_execution(
    state: &mut SupervisorState,
    output_rx: &mut broadcast::Receiver<AgentOutput>,
    running: bool,
) {
    let mut active = running;
    loop {
        let progress = run_execution_iteration(state, output_rx, active).await;
        if matches!(progress, ExecutionProgress::Stop) {
            return;
        }
        // Preserve running state across iterations; `active` is not externally
        // updated during begin_execution since the command loop is blocked here.
        active = true;
    }
}

// ── Step helpers ──────────────────────────────────────────────────────────────

/// Extracts the next pending leaf from the active plan, if one exists.
fn next_leaf(state: &SupervisorState) -> Option<LeafInfo> {
    let arc = state.active_plan.as_ref()?;
    let node = arc.next_pending_leaf()?.clone();
    let plan_id = arc.id.clone();
    Some(LeafInfo { node, plan_id })
}

/// Reads the step file for `leaf`, falling back to the node title if missing.
async fn load_step_prompt(state: &SupervisorState, leaf: &LeafInfo) -> String {
    let step_file = match leaf.node.config.step_file.as_deref() {
        Some(f) => f,
        None => return leaf.node.title.to_string(),
    };
    let step_file = StepFileName::new(step_file);
    match state.store.read_step(&leaf.plan_id, &step_file).await {
        Ok(content) => content.into_inner(),
        Err(e) => {
            warn!(error = %e, node_id = %leaf.node.id, "could not read step file, using title");
            leaf.node.title.to_string()
        }
    }
}

/// Drains the executor output until `TurnComplete`, accumulating `StepOutcome`.
async fn drain_step_output(
    state: &mut SupervisorState,
    output_rx: &mut broadcast::Receiver<AgentOutput>,
) -> StepOutcome {
    let mut outcome = StepOutcome::default();
    loop {
        match process_step_output_event(state, output_rx.recv().await, &mut outcome) {
            DrainSignal::Continue => {}
            DrainSignal::Complete => break,
            DrainSignal::ChannelClosed => {
                outcome.has_error = IsPredicate::yes();
                outcome.error_message = Some(OutputText::from("executor output channel closed"));
                break;
            }
        }
    }
    outcome
}

async fn handle_non_start_supervisor_command(
    state: &mut SupervisorState,
    cmd: SupervisorCmd,
    running: &mut bool,
) -> bool {
    match cmd {
        SupervisorCmd::Stop => true,
        pause_or_resume @ SupervisorCmd::Pause | pause_or_resume @ SupervisorCmd::Resume => {
            apply_pause_or_resume(running, pause_or_resume);
            false
        }
        SupervisorCmd::CancelPlan => {
            handle_cancel_plan(state).await;
            *running = true;
            false
        }
        other => handle_misc_non_start_command(state, other),
    }
}

fn apply_pause_or_resume(running: &mut bool, cmd: SupervisorCmd) {
    match cmd {
        SupervisorCmd::Pause => {
            *running = false;
            debug!("SupervisorActor: paused");
        }
        SupervisorCmd::Resume => {
            *running = true;
            debug!("SupervisorActor: resumed");
        }
        _ => {}
    }
}

fn handle_misc_non_start_command(state: &mut SupervisorState, cmd: SupervisorCmd) -> bool {
    if let SupervisorCmd::InjectStep { parent_id, node } = cmd {
        handle_inject_step(state, parent_id, node);
    }
    false
}

async fn handle_start_plan_command(
    state: &mut SupervisorState,
    goal: GoalText,
    running: bool,
) -> bool {
    if state.active_plan.is_some() {
        warn!("StartPlan received while plan is already running - ignoring");
        return false;
    }
    handle_start_plan(state, goal, running).await;
    false
}

enum ExecutionProgress {
    Continue,
    Stop,
}

async fn run_execution_iteration(
    state: &mut SupervisorState,
    output_rx: &mut broadcast::Receiver<AgentOutput>,
    active: bool,
) -> ExecutionProgress {
    if !active {
        debug!("SupervisorActor: execution paused - halting step dispatch");
        return ExecutionProgress::Stop;
    }

    let Some(leaf) = next_leaf(state) else {
        info!("SupervisorActor: all steps complete");
        emit(&state.event_tx, SupervisorEvent::ExecutionComplete);
        return ExecutionProgress::Stop;
    };

    emit(
        &state.event_tx,
        SupervisorEvent::StepStarted(leaf.node.id.clone()),
    );

    let prompt = load_step_prompt(state, &leaf).await;
    state.executor.set_mode(ExecutorMode::Plan).await;
    state.executor.send_prompt(PromptText::from(prompt)).await;

    let outcome = drain_step_output(state, output_rx).await;
    let gate = evaluate_gate(&leaf.node, &outcome);
    if bool::from(gate.passed) {
        complete_step(state, &leaf).await;
        maybe_checkpoint(state, &leaf, output_rx).await;
        return ExecutionProgress::Continue;
    }

    let reason = gate
        .reason
        .unwrap_or_else(|| OutputText::new("unknown failure"));
    fail_step(state, &leaf, reason).await;
    ExecutionProgress::Stop
}

enum DrainSignal {
    Continue,
    Complete,
    ChannelClosed,
}

fn process_step_output_event(
    state: &mut SupervisorState,
    received: Result<AgentOutput, broadcast::error::RecvError>,
    outcome: &mut StepOutcome,
) -> DrainSignal {
    match received {
        Ok(output) => process_agent_output_event(state, output, outcome),
        Err(_) => DrainSignal::ChannelClosed,
    }
}

fn process_agent_output_event(
    state: &mut SupervisorState,
    output: AgentOutput,
    outcome: &mut StepOutcome,
) -> DrainSignal {
    if matches!(output, AgentOutput::TurnComplete) {
        return DrainSignal::Complete;
    }
    record_last_plan_node_status(outcome, &output);
    maybe_forward_display_output(state, output);
    DrainSignal::Continue
}

fn record_last_plan_node_status(outcome: &mut StepOutcome, output: &AgentOutput) {
    if let AgentOutput::PlanNodeUpdate {
        node_id,
        status,
        notes: _,
    } = output
    {
        outcome.last_node_status = Some((node_id.clone(), status.clone()));
    }
}

fn maybe_forward_display_output(state: &mut SupervisorState, output: AgentOutput) {
    if matches!(
        output,
        AgentOutput::IntentMessage(_)
            | AgentOutput::ToolProgress { .. }
            | AgentOutput::ToolPartialResult { .. }
    ) {
        emit(&state.event_tx, SupervisorEvent::DisplayOutput(output));
    }
}

/// Drains the executor output after a compact command until `TurnComplete`.
///
/// After `executor.compact()` the executor emits a `TurnComplete` from the
/// compact operation. Without draining it, the next step's drain loop would
/// exit immediately and fail the gate. This helper clears that signal.
async fn drain_compact(output_rx: &mut broadcast::Receiver<AgentOutput>) {
    loop {
        match output_rx.recv().await {
            Ok(AgentOutput::TurnComplete) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

/// Applies `Done` status to `leaf.node`, saves the plan, and broadcasts `StepCompleted`.
async fn complete_step(state: &mut SupervisorState, leaf: &LeafInfo) {
    update_node_status(state, &leaf.node.id, NodeStatus::Done);
    state.checkpoint.record_file_change();
    save_active_plan(state).await;
    emit(
        &state.event_tx,
        SupervisorEvent::StepCompleted(leaf.node.id.clone()),
    );
    debug!(node_id = %leaf.node.id, "step completed");
}

/// Applies `Failed` status, saves the plan, and broadcasts `StepFailed`.
async fn fail_step(state: &mut SupervisorState, leaf: &LeafInfo, reason: OutputText) {
    update_node_status(
        state,
        &leaf.node.id,
        NodeStatus::Failed(FailureReason::from(reason.to_string())),
    );
    save_active_plan(state).await;
    emit(
        &state.event_tx,
        SupervisorEvent::StepFailed {
            id: leaf.node.id.clone(),
            reason,
        },
    );
    warn!(node_id = %leaf.node.id, "step failed");
}

/// Fires a checkpoint if `should_trigger` and resets the tracker afterwards.
async fn maybe_checkpoint(
    state: &mut SupervisorState,
    leaf: &LeafInfo,
    output_rx: &mut broadcast::Receiver<AgentOutput>,
) {
    let config = leaf.node.config.checkpoint.as_ref();
    let should_fire = bool::from(state.checkpoint.should_trigger(config));
    if !should_fire {
        return;
    }
    let config_clone = leaf.node.config.checkpoint.clone().unwrap_or(
        augur_domain::domain::plan_tree::CheckpointConfig {
            commit: false.into(),
            compact: true.into(),
        },
    );
    emit(
        &state.event_tx,
        SupervisorEvent::CheckpointTriggered(config_clone),
    );
    state.executor.compact().await;
    drain_compact(output_rx).await;
    state.checkpoint.reset();
}

/// Applies `status` to the node with `id` via `Arc::make_mut`.
///
/// `Arc::make_mut` clones the tree only when other strong references exist.
/// The supervisor is the sole long-lived reference; TUI clones are transient,
/// so the clone is rare in practice.
fn update_node_status(state: &mut SupervisorState, id: &PlanNodeId, status: NodeStatus) {
    if let Some(arc) = state.active_plan.as_mut() {
        Arc::make_mut(arc).update_node_status(id, status);
    }
}

/// Saves the active plan tree to disk; logs a warning on failure.
async fn save_active_plan(state: &mut SupervisorState) {
    if let Some(arc) = state.active_plan.as_ref()
        && let Err(e) = state.store.save(arc).await
    {
        warn!(error = %e, "SupervisorActor: failed to save plan tree");
    }
}

/// Broadcasts `event` on the event channel; ignores send errors (no subscribers).
fn emit(tx: &broadcast::Sender<SupervisorEvent>, event: SupervisorEvent) {
    let _ = tx.send(event);
}

async fn run_meta_plan(
    tree: &mut PlanTree,
    output_rx: &mut broadcast::Receiver<AgentOutput>,
) -> Result<(), MetaPlanError> {
    loop {
        match output_rx.recv().await {
            Ok(output) => {
                if matches!(
                    apply_meta_output(tree, output),
                    super::meta_planner::MetaTurnProgress::Complete
                ) {
                    return Ok(());
                }
            }
            Err(_) => return Err(MetaPlanError::ChannelClosed),
        }
    }
}
