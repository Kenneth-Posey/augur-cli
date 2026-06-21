//! Deterministic orchestrator runtime actor composition.

mod deterministic_orchestrator_ops;
mod failure_routing;
mod parallel_groups;
mod progression;
use deterministic_orchestrator_ops::{
    annotate_last_failure_decision, apply_artifact_updates, dispatch_request, emit, emit_halted,
    emit_step_progress, handle_evaluator_dispatch_failure, handle_worker_dispatch_failure,
    merge_artifact_updates, StepProgressArgs,
};

use super::artifact_store::{ArtifactUpdate, StepArtifactResolver};
use super::background_dispatch::{
    AgentDispatchTicket, BackgroundAgentRuntime, DeterministicAgentDispatcher,
};
use super::commands::DeterministicOrchestratorCmd;
use super::decision::{
    choose_failure_decision, DefaultFailureDecisionPolicy, FailureDecisionInput,
    FailureDecisionPolicy,
};
use super::handle::DeterministicOrchestratorHandle;
use super::loader::{ensure_local_workflow_file, load_workflow_document};
use crate::domain::deterministic_orchestrator::{
    DeterministicOrchestratorEvent, FailureDecision, FailureOrigin, GroupMemberResult,
    NormalizedSignal, PendingFailureContext, StepEvaluatorRecord, StepExecutionRecord,
    WorkflowRunState, WorkflowStep, WorkflowStepKind,
};
use crate::domain::deterministic_orchestrator_ops::{
    build_evaluator_dispatch_request, build_patch_dispatch_request, build_step_index,
    build_worker_dispatch_request, derive_feature_slug, resolve_failure_transition,
    resolve_pass_transition, DispatchRequestKind, ExecutedStepIndex, FailureTransitionContext,
    FailureTransitionResolution, PassTransitionResolution, StepIndex, WorkflowDispatchRequest,
};
use augur_domain::domain::types::{AutomatedUserMessage, FeedEntry};
use augur_domain::domain::{
    AgentName, FeatureContext, FeatureSlug, OutputText, StringNewtype, WorkflowStepId,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

const DETERMINISTIC_ORCHESTRATOR_CMD_CAPACITY: usize = 32;
const DETERMINISTIC_ORCHESTRATOR_EVENT_CAPACITY: usize = 64;
const DETERMINISTIC_ORCHESTRATOR_AUTO_MSG_CAPACITY: usize = 64;

#[derive(Clone)]
struct RuntimePorts {
    cmd_tx: mpsc::Sender<DeterministicOrchestratorCmd>,
    event_tx: broadcast::Sender<DeterministicOrchestratorEvent>,
    agent_feed_tx: Option<mpsc::Sender<FeedEntry>>,
    dispatch_runtime: Arc<dyn BackgroundAgentRuntime>,
    auto_msg_tx: broadcast::Sender<AutomatedUserMessage>,
}

#[derive(Clone, Debug, bon::Builder)]
struct StepOutcome {
    step_id: WorkflowStepId,
    signal: NormalizedSignal,
    artifact_updates: Vec<ArtifactUpdate>,
    /// Evaluator response text captured when the evaluator emitted Hold.
    /// `None` for worker completions and evaluator Advance results.
    evaluator_output: Option<OutputText>,
}

impl StepOutcome {
    /// Convenience constructor for worker completions where no evaluator output is present.
    fn new(
        step_id: WorkflowStepId,
        signal: NormalizedSignal,
        artifact_updates: Vec<ArtifactUpdate>,
    ) -> Self {
        Self {
            step_id,
            signal,
            artifact_updates,
            evaluator_output: None,
        }
    }
}

#[derive(Clone, Debug)]
struct AppliedDecision {
    step_id: WorkflowStepId,
    decision: Option<FailureDecision>,
}

#[derive(Clone, Debug)]
struct PendingStepExecution {
    execution: StepExecutionRecord,
    artifact_updates: Vec<ArtifactUpdate>,
}

#[derive(Clone, Debug)]
struct EvaluatedStep {
    step: WorkflowStep,
    execution: StepExecutionRecord,
    transition_signal: NormalizedSignal,
    failure_origin: FailureOrigin,
    artifact_updates: Vec<ArtifactUpdate>,
}

struct RunLoopArgs {
    cmd_rx: mpsc::Receiver<DeterministicOrchestratorCmd>,
    ports: RuntimePorts,
    repo_root: PathBuf,
    failure_policy: Arc<dyn FailureDecisionPolicy>,
}

/// Bundled arguments for pipeline start, combining context and slug to keep
/// `handle_start` within the three-parameter limit.
struct PipelineStartArgs {
    feature_context: Option<FeatureContext>,
    feature_slug: Option<FeatureSlug>,
    resume: bool,
}

/// Bundles a step with its pending execution record for evaluator dispatch.
struct DispatchableStep {
    step: WorkflowStep,
    pending: PendingStepExecution,
}

/// Routing outcome for a step that emitted `NeedsRevision` with a configured
/// `on_needs_revision` action.
#[derive(Clone, Debug, PartialEq, Eq)]
enum NeedsRevisionRouting {
    /// First attempt - remediation has not been tried for this step yet.
    /// Phase 4 will dispatch the remediation agent; Phase 3 is fail-closed (Hold).
    Remediate,
    /// Remediation was already attempted for this step - fall back to `on_fail`.
    HoldCycleGuard,
}

#[derive(Clone, Debug)]
struct AgentExecutionFailure {
    step_id: WorkflowStepId,
    dispatch_kind: DispatchRequestKind,
}

impl AgentExecutionFailure {
    fn new(step_id: WorkflowStepId, dispatch_kind: DispatchRequestKind) -> Self {
        Self {
            step_id,
            dispatch_kind,
        }
    }
}

#[derive(Clone, Debug)]
struct DeclaredStepTransition {
    from_step_id: WorkflowStepId,
    target_step_id: WorkflowStepId,
}

struct FailureResolutionContext {
    applied: AppliedDecision,
    step: WorkflowStep,
    resolution: FailureTransitionResolution,
}

struct CompletionForwarderArgs {
    dispatcher: DeterministicAgentDispatcher,
    ticket: AgentDispatchTicket,
    artifact_store: StepArtifactResolver,
    request: WorkflowDispatchRequest,
}

struct EvaluatorDispatchFailure {
    step: WorkflowStep,
    step_id: WorkflowStepId,
}

/// Bundled arguments for the quick-patch dispatch path.
#[derive(bon::Builder)]
struct DelegateFixArgs {
    /// Step ID of the failing reviewer (used for error events).
    step_id: WorkflowStepId,
    /// Quick-patch agent to dispatch.
    patch_agent: AgentName,
    /// Reviewer step to re-dispatch after the patch completes.
    return_to_reviewer: WorkflowStepId,
    /// Step-failure attempt number for logging and future cap enforcement.
    attempt: u8,
    /// Full reviewer output text passed to the patch agent.
    failure_notes: Option<OutputText>,
}

/// Workflow position tracking (current step index + history of executed steps).
///
/// Also stores the feature slug and context for the duration of the pipeline run.
struct WorkflowProgress {
    step_index: StepIndex,
    executed_steps: ExecutedStepIndex,
    feature_slug: Option<FeatureSlug>,
    feature_context: Option<FeatureContext>,
}

impl WorkflowProgress {
    fn new() -> Self {
        Self {
            step_index: StepIndex::default(),
            executed_steps: ExecutedStepIndex::default(),
            feature_slug: None,
            feature_context: None,
        }
    }
}

/// Actor-owned mutable runtime state for deterministic orchestration.
struct DeterministicOrchestratorRunState {
    progress: WorkflowProgress,
    run_state: WorkflowRunState,
    pending_worker: Option<PendingStepExecution>,
    artifact_store: StepArtifactResolver,
    failure_policy: Arc<dyn FailureDecisionPolicy>,
}

impl DeterministicOrchestratorRunState {
    fn new(repo_root: PathBuf, failure_policy: Arc<dyn FailureDecisionPolicy>) -> Self {
        Self {
            progress: WorkflowProgress::new(),
            run_state: WorkflowRunState::default(),
            pending_worker: None,
            artifact_store: StepArtifactResolver::new(repo_root),
            failure_policy,
        }
    }
}

/// Spawns the deterministic orchestrator actor task and returns its public handle.
pub fn spawn(repo_root: impl Into<PathBuf>) -> DeterministicOrchestratorHandle {
    spawn_with_join(repo_root).1
}

/// Spawns the orchestrator actor with the default failure policy and no agent
/// feed channel, returning both the `JoinHandle` for the background task and
/// the `DeterministicOrchestratorHandle` used to communicate with the actor.
pub(crate) fn spawn_with_join(
    repo_root: impl Into<PathBuf>,
) -> (JoinHandle<()>, DeterministicOrchestratorHandle) {
    spawn_with_join_and_policy(
        repo_root,
        SpawnPolicyArgs::new(Arc::new(DefaultFailureDecisionPolicy)),
    )
}

/// Spawns the orchestrator actor wired to the supplied `failure_policy` and an
/// optional `agent_feed_tx` channel, returning both the `JoinHandle` for the
/// background task and the `DeterministicOrchestratorHandle` for command dispatch.
///
/// # Parameters
/// - `repo_root`: Filesystem path used as the root for artifact resolution.
/// - `failure_policy`: Governs step-failure decisions (retry, abort, skip).
/// - `agent_feed_tx`: When `Some`, agent feed events are forwarded to this sender.
pub(crate) struct SpawnPolicyArgs {
    failure_policy: Arc<dyn FailureDecisionPolicy>,
    agent_feed_tx: Option<mpsc::Sender<FeedEntry>>,
    dispatch_runtime: Arc<dyn BackgroundAgentRuntime>,
}

impl SpawnPolicyArgs {
    fn new(failure_policy: Arc<dyn FailureDecisionPolicy>) -> Self {
        Self {
            failure_policy,
            agent_feed_tx: None,
            dispatch_runtime: Arc::new(
                super::background_dispatch::MissingBackgroundAgentRuntime {},
            ),
        }
    }

    fn with_agent_feed(mut self, agent_feed_tx: mpsc::Sender<FeedEntry>) -> Self {
        self.agent_feed_tx = Some(agent_feed_tx);
        self
    }

    fn with_dispatch_runtime(mut self, dispatch_runtime: Arc<dyn BackgroundAgentRuntime>) -> Self {
        self.dispatch_runtime = dispatch_runtime;
        self
    }
}

/// Spawn a `DeterministicOrchestratorActor` with a join handle and an attached failure policy.
///
/// Creates all internal channels, constructs the actor with the given
/// `SpawnPolicyArgs`, and returns both the `JoinHandle` for the actor task
/// and a `DeterministicOrchestratorHandle` for interacting with it.
pub(crate) fn spawn_with_join_and_policy(
    repo_root: impl Into<PathBuf>,
    args: SpawnPolicyArgs,
) -> (JoinHandle<()>, DeterministicOrchestratorHandle) {
    let (cmd_tx, cmd_rx) =
        mpsc::channel::<DeterministicOrchestratorCmd>(DETERMINISTIC_ORCHESTRATOR_CMD_CAPACITY);
    let (event_tx, _) = broadcast::channel::<DeterministicOrchestratorEvent>(
        DETERMINISTIC_ORCHESTRATOR_EVENT_CAPACITY,
    );
    let (auto_msg_tx, _) =
        broadcast::channel::<AutomatedUserMessage>(DETERMINISTIC_ORCHESTRATOR_AUTO_MSG_CAPACITY);
    let repo_root = repo_root.into();
    let handle =
        DeterministicOrchestratorHandle::new(cmd_tx, event_tx.clone(), auto_msg_tx.clone());
    let join = tokio::spawn(run_loop(RunLoopArgs {
        cmd_rx,
        ports: RuntimePorts {
            cmd_tx: handle.cmd_tx.clone(),
            event_tx,
            agent_feed_tx: args.agent_feed_tx,
            dispatch_runtime: args.dispatch_runtime,
            auto_msg_tx,
        },
        repo_root,
        failure_policy: args.failure_policy,
    }));
    (join, handle)
}

/// Spawns the orchestrator wired to the shared agent-feed channel.
///
/// Inputs:
/// - `repo_root`: repository root for workflow file resolution.
/// - `agent_feed_tx`: sending half of the shared agent-feed mpsc channel;
///   all agent feed events from dispatched agents are teed to this channel.
///
/// Returns:
/// - `(JoinHandle<()>, DeterministicOrchestratorHandle)` for the spawned task.
pub fn spawn_with_join_and_feed(
    repo_root: impl Into<PathBuf>,
    agent_feed_tx: mpsc::Sender<FeedEntry>,
) -> (JoinHandle<()>, DeterministicOrchestratorHandle) {
    spawn_with_join_and_policy(
        repo_root,
        SpawnPolicyArgs::new(Arc::new(DefaultFailureDecisionPolicy)).with_agent_feed(agent_feed_tx),
    )
}

/// Spawns the orchestrator wired to both feed and a provider-owned dispatch runtime.
pub fn spawn_with_join_and_feed_and_runtime(
    repo_root: impl Into<PathBuf>,
    agent_feed_tx: mpsc::Sender<FeedEntry>,
    dispatch_runtime: Arc<dyn BackgroundAgentRuntime>,
) -> (JoinHandle<()>, DeterministicOrchestratorHandle) {
    spawn_with_join_and_policy(
        repo_root,
        SpawnPolicyArgs::new(Arc::new(DefaultFailureDecisionPolicy))
            .with_agent_feed(agent_feed_tx)
            .with_dispatch_runtime(dispatch_runtime),
    )
}

/// Receives runtime commands and coordinates deterministic workflow execution.
async fn run_loop(mut args: RunLoopArgs) {
    let mut state = DeterministicOrchestratorRunState::new(args.repo_root, args.failure_policy);

    while let Some(cmd) = args.cmd_rx.recv().await {
        if !handle_command(&mut state, &args.ports, cmd).await {
            break;
        }
    }
}

async fn handle_command(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    cmd: DeterministicOrchestratorCmd,
) -> bool {
    match cmd {
        DeterministicOrchestratorCmd::Start {
            feature_context,
            feature_slug,
            resume,
        } => {
            handle_start(
                state,
                ports,
                PipelineStartArgs {
                    feature_context: feature_context.map(FeatureContext::from),
                    feature_slug: feature_slug.map(FeatureSlug::from),
                    resume,
                },
            )
            .await;
        }
        DeterministicOrchestratorCmd::Shutdown => return false,
        cmd => handle_runtime_update(state, ports, cmd).await,
    }

    true
}

async fn handle_runtime_update(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    cmd: DeterministicOrchestratorCmd,
) {
    if let Some(worker_outcome) = worker_completion_outcome(cmd.clone()) {
        handle_worker_completion(state, ports, worker_outcome).await;
        return;
    }
    if let Some(evaluator_outcome) = evaluator_completion_outcome(cmd.clone()) {
        handle_evaluator_completion(state, ports, evaluator_outcome).await;
        return;
    }
    if let Some(applied) = applied_failure_decision(cmd.clone()) {
        failure_routing::apply_failure_policy(state, ports, applied).await;
        return;
    }
    if let Some(failure) = agent_execution_failure(cmd.clone()) {
        handle_agent_execution_failure(state, ports, failure).await;
        return;
    }
    unreachable!("handle_command routes start/shutdown before runtime updates");
}

fn worker_completion_outcome(cmd: DeterministicOrchestratorCmd) -> Option<StepOutcome> {
    if let DeterministicOrchestratorCmd::WorkerCompleted {
        step_id,
        signal,
        artifact_updates,
    } = cmd
    {
        return Some(StepOutcome::new(step_id, signal, artifact_updates));
    }
    None
}

fn evaluator_completion_outcome(cmd: DeterministicOrchestratorCmd) -> Option<StepOutcome> {
    if let DeterministicOrchestratorCmd::EvaluatorCompleted {
        step_id,
        signal,
        artifact_updates,
        evaluator_output,
    } = cmd
    {
        return Some(
            StepOutcome::builder()
                .step_id(step_id)
                .signal(signal)
                .artifact_updates(artifact_updates)
                .maybe_evaluator_output(evaluator_output)
                .build(),
        );
    }
    None
}

fn applied_failure_decision(cmd: DeterministicOrchestratorCmd) -> Option<AppliedDecision> {
    if let DeterministicOrchestratorCmd::ApplyFailureDecision { step_id, decision } = cmd {
        return Some(AppliedDecision {
            step_id,
            decision: Some(decision),
        });
    }
    None
}

fn agent_execution_failure(cmd: DeterministicOrchestratorCmd) -> Option<AgentExecutionFailure> {
    if let DeterministicOrchestratorCmd::AgentExecutionFailed { step_id, kind } = cmd {
        return Some(AgentExecutionFailure::new(step_id, kind));
    }
    None
}

/// Resets and applies the feature identity fields of pipeline progress state.
///
/// Clears any stale slug/context from a prior run, then populates from the given
/// optional values. If both context and slug are provided, the slug wins; if only
/// context is given, the slug is derived from the context string.
fn apply_feature_identity(
    state: &mut DeterministicOrchestratorRunState,
    feature_context: Option<FeatureContext>,
    feature_slug: Option<FeatureSlug>,
) {
    state.progress.feature_slug = None;
    state.progress.feature_context = None;
    if let Some(ctx) = feature_context {
        let slug = feature_slug.unwrap_or_else(|| derive_feature_slug(&ctx));
        state.progress.feature_context = Some(ctx);
        state.progress.feature_slug = Some(slug);
    } else if let Some(slug) = feature_slug {
        state.progress.feature_slug = Some(slug);
    }
}

/// Resets all mutable pipeline run fields and sets the initial current step.
///
/// Inputs: `step_index` - newly built index; `first_step_id` - first step or `None`
/// when the workflow is empty.
fn initialize_pipeline_run(
    state: &mut DeterministicOrchestratorRunState,
    step_index: StepIndex,
    first_step_id: Option<WorkflowStepId>,
) {
    state.progress.step_index = step_index;
    state.progress.executed_steps = ExecutedStepIndex::default();
    state.run_state = WorkflowRunState::default();
    state.pending_worker = None;
    state.run_state.current_step_id = first_step_id;
}

async fn handle_start(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    args: PipelineStartArgs,
) {
    let resume = args.resume;
    apply_feature_identity(state, args.feature_context, args.feature_slug);

    if ensure_local_workflow_file(state.artifact_store.repo_root()).is_err() {
        tracing::warn!("deterministic orchestrator failed to seed local workflow file");
        return;
    }

    let Ok(document) = load_workflow_document(state.artifact_store.repo_root()) else {
        tracing::warn!("deterministic orchestrator failed to load local workflow document");
        return;
    };

    let mut step_index = build_step_index(&document);

    if let Some(slug) = state.progress.feature_slug.clone() {
        apply_slug_to_step_index(&mut step_index, &slug);
    }

    let first_step_id = if resume {
        let resume_step = state.artifact_store.find_resume_step_id(&step_index);
        tracing::info!(step_id = ?resume_step, "pipeline resuming");
        resume_step
    } else {
        step_index.ordered_executable_step_ids.first().cloned()
    };

    let emit_first_step_id = first_step_id.clone();
    initialize_pipeline_run(state, step_index, first_step_id);

    emit(
        &ports.event_tx,
        DeterministicOrchestratorEvent::Started {
            first_step_id: emit_first_step_id.clone(),
        },
    );

    if emit_first_step_id.is_none() {
        emit(&ports.event_tx, DeterministicOrchestratorEvent::Completed);
        return;
    }

    start_current_step(state, ports).await;
}

/// Replaces `<feature-slug>` placeholders in all step artifact paths within the index.
///
/// Inputs:
/// - `step_index`: mutable step index whose stored step paths are updated in place.
/// - `slug`: derived feature slug substituted for every `<feature-slug>` occurrence.
///
/// Side effects:
/// - Mutates `expected_inputs` and `created_artifacts` paths in every stored step.
fn apply_slug_to_step_index(step_index: &mut StepIndex, slug: &FeatureSlug) {
    step_index.apply_slug(slug);
}

async fn start_current_step(state: &mut DeterministicOrchestratorRunState, ports: &RuntimePorts) {
    progression::start_current_step(state, ports).await;
}

async fn handle_worker_completion(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    completion: StepOutcome,
) {
    progression::handle_worker_completion(state, ports, completion).await;
}

async fn handle_evaluator_completion(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    completion: StepOutcome,
) {
    progression::handle_evaluator_completion(state, ports, completion).await;
}

fn same_step_id(left: &WorkflowStepId, right: &WorkflowStepId) -> Option<()> {
    (left == right).then_some(())
}

async fn handle_agent_execution_failure(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    failure: AgentExecutionFailure,
) {
    failure_routing::handle_agent_execution_failure(state, ports, failure).await;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MockRemediationDispatch {
    request_step_id: WorkflowStepId,
    agent_name: AgentName,
    signal: NormalizedSignal,
}

impl MockRemediationDispatch {
    fn new(
        request_step_id: WorkflowStepId,
        agent_name: AgentName,
        signal: NormalizedSignal,
    ) -> Self {
        Self {
            request_step_id,
            agent_name,
            signal,
        }
    }
}

tokio::task_local! {
    static MOCK_REMEDIATION_DISPATCHES: std::cell::RefCell<
        std::collections::VecDeque<MockRemediationDispatch>
    >;
}

async fn with_mock_remediation_dispatches<Fut, T>(
    dispatches: Vec<MockRemediationDispatch>,
    future: Fut,
) -> T
where
    Fut: std::future::Future<Output = T>,
{
    MOCK_REMEDIATION_DISPATCHES
        .scope(std::cell::RefCell::new(dispatches.into()), async move {
            let result = future.await;
            assert!(
                MOCK_REMEDIATION_DISPATCHES.with(|queue| queue.borrow().is_empty()),
                "all mocked remediation dispatches should be consumed",
            );
            result
        })
        .await
}

fn latest_parallel_group_member_results(
    state: &DeterministicOrchestratorRunState,
) -> Option<&[GroupMemberResult]> {
    parallel_groups::latest_parallel_group_member_results(state)
}

fn build_member_retry_dispatch_request(
    state: &DeterministicOrchestratorRunState,
    member_result: &GroupMemberResult,
) -> Option<WorkflowDispatchRequest> {
    parallel_groups::build_member_retry_dispatch_request(state, member_result)
}

/// Routes a `NeedsRevision` signal through the cycle guard, then executes two-phase
/// remediation dispatch or falls through to `automatically_apply_failure_policy`.
///
/// On [`NeedsRevisionRouting::Remediate`]: sets `remediation_attempted = true` on
/// the last step record so that a second `NeedsRevision` from the same step is
/// caught by the cycle guard, then dispatches the two-phase remediation sequence.
/// If remediation succeeds, advances through the step's `on_pass` path.
/// If remediation fails, falls through to `automatically_apply_failure_policy`.
///
/// On [`NeedsRevisionRouting::HoldCycleGuard`]: passes directly to
/// `automatically_apply_failure_policy` without re-marking the record, preventing
/// an infinite fix → validate → fail loop.
///
/// Inputs:
/// - `state`: mutable run state updated with the remediation flag and advance cursor.
/// - `ports`: runtime ports for event emission and dispatcher construction.
/// - `step`: the workflow step that emitted `NeedsRevision`.
///
/// Side effects:
/// - Sets `remediation_attempted = true` on the last prior-steps record (Remediate path).
/// - May advance the pipeline cursor and emit pass-path events on remediation success.
/// - Calls `automatically_apply_failure_policy` on remediation failure or cycle guard.
async fn apply_needs_revision_routing(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    step: &WorkflowStep,
) {
    failure_routing::apply_needs_revision_routing(state, ports, step).await;
}

/// Records artifact updates, pushes the execution record, and tracks group membership.
///
/// Inputs: `state` - run state updated with artifacts and prior step log;
/// `evaluated` - the evaluated step carrying execution data and artifact changes.
fn record_evaluated_step_state(
    state: &mut DeterministicOrchestratorRunState,
    evaluated: &EvaluatedStep,
) {
    apply_artifact_updates(state, &evaluated.execution, &evaluated.artifact_updates);
    state
        .run_state
        .prior_steps
        .push(evaluated.execution.clone());
    state
        .progress
        .executed_steps
        .record_execution(&evaluated.execution.step_id);
    record_parallel_group_member_result(state, evaluated);
}

/// Sets failure context and routes the step via its configured failure/revision policy.
///
/// Inputs: `state` - run state mutated with pending failure; `ports` - channels for
/// dispatch events; `evaluated` - the failed evaluated step.
async fn handle_step_failure(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    evaluated: &EvaluatedStep,
) {
    state.run_state.pending_failure = Some(
        PendingFailureContext::builder()
            .step_id(evaluated.step.id.clone())
            .last_signal(evaluated.transition_signal.clone())
            .origin(evaluated.failure_origin.clone())
            .maybe_failure_notes(
                evaluated
                    .execution
                    .evaluator_record
                    .evaluator_output
                    .clone(),
            )
            .build(),
    );
    let is_needs_revision = evaluated.transition_signal == NormalizedSignal::NeedsRevision;
    let has_configured_revision_action = evaluated
        .step
        .transition
        .on_needs_revision
        .action
        .uses_declared_automatic_transition()
        .0;
    if is_needs_revision && has_configured_revision_action {
        apply_needs_revision_routing(state, ports, &evaluated.step).await;
    } else {
        failure_routing::automatically_apply_failure_policy(state, ports, &evaluated.step).await;
    }
}

/// Sends the automated pass message on the auto-message channel.
///
/// Inputs: `ports` - channels including `auto_msg_tx`; `evaluated` - the evaluated step
/// whose id and artifact updates are rendered into the message text.
async fn broadcast_step_pass_message(ports: &RuntimePorts, evaluated: &EvaluatedStep) {
    let artifact_paths: Vec<&str> = evaluated
        .artifact_updates
        .iter()
        .map(|u| u.artifact.path.as_str())
        .collect();
    let msg_text = if artifact_paths.is_empty() {
        format!("Step '{}' passed.", evaluated.step.id)
    } else {
        format!(
            "Step '{}' passed. Artifacts: {}.",
            evaluated.step.id,
            artifact_paths.join(", ")
        )
    };
    let _ = ports
        .auto_msg_tx
        .send(AutomatedUserMessage(OutputText::new(msg_text)));
}

/// Routes a passing step: handles group-member advancement or resolves the pass transition.
///
/// Inputs: `state` - run state; `ports` - channels; `evaluated` - the passing evaluated step.
async fn route_step_after_pass(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    evaluated: &EvaluatedStep,
) {
    // Group members do not own their own on_pass transition - the group step does.
    if evaluated.step.kind == WorkflowStepKind::GroupMember {
        advance_parallel_group_or_next_member(state, ports, &evaluated.step).await;
        return;
    }
    match resolve_pass_transition(&evaluated.step, &evaluated.transition_signal) {
        PassTransitionResolution::AdvanceTo(next_step_id) => {
            transition_to_declared_step_target(
                state,
                ports,
                DeclaredStepTransition {
                    from_step_id: evaluated.step.id.clone(),
                    target_step_id: next_step_id,
                },
            )
            .await;
        }
        PassTransitionResolution::Complete => {
            state.run_state.current_step_id = None;
            emit(&ports.event_tx, DeterministicOrchestratorEvent::Completed);
        }
        PassTransitionResolution::StayOnCurrentStep => {
            Box::pin(start_current_step(state, ports)).await;
        }
    }
}

async fn handle_step_evaluation(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    evaluated: EvaluatedStep,
) {
    state.pending_worker = None;
    let step_passed = evaluated.transition_signal == NormalizedSignal::Advance;
    record_evaluated_step_state(state, &evaluated);

    if !step_passed {
        handle_step_failure(state, ports, &evaluated).await;
        return;
    }

    state.run_state.pending_failure = None;
    broadcast_step_pass_message(ports, &evaluated).await;
    route_step_after_pass(state, ports, &evaluated).await;
}

/// Advances a parallel group after one of its members completes with a pass signal.
/// Dispatches the next undispatched member if one exists; otherwise resolves via the
/// group step's own `on_pass` transition, which is the only place the `RUN_COMPLETE`
/// sentinel or a group-level `next_step` should appear.
async fn advance_parallel_group_or_next_member(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    member_step: &WorkflowStep,
) {
    parallel_groups::advance_parallel_group_or_next_member(state, ports, member_step).await;
}

fn record_parallel_group_member_result(
    state: &mut DeterministicOrchestratorRunState,
    evaluated: &EvaluatedStep,
) {
    parallel_groups::record_parallel_group_member_result(state, evaluated);
}

struct BacktrackTarget {
    from_step_id: WorkflowStepId,
    target_step_id: WorkflowStepId,
}

async fn transition_to_declared_step_target(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    transition: DeclaredStepTransition,
) {
    let Some(resolved_step_id) = state
        .progress
        .step_index
        .resolve_transition_target_step_id(&transition.target_step_id)
    else {
        state.run_state.current_step_id = None;
        state.pending_worker = None;
        state.run_state.pending_failure = None;
        emit_halted(&ports.event_tx, transition.from_step_id);
        return;
    };

    state.run_state.current_step_id = Some(resolved_step_id);
    Box::pin(start_current_step(state, ports)).await;
}

fn current_step(state: &DeterministicOrchestratorRunState) -> Option<&WorkflowStep> {
    let current_step_id = state.run_state.current_step_id.as_ref()?;
    workflow_step(state, current_step_id)
}

fn workflow_step<'a>(
    state: &'a DeterministicOrchestratorRunState,
    step_id: &WorkflowStepId,
) -> Option<&'a WorkflowStep> {
    state.progress.step_index.workflow_step(step_id)
}

fn worker_execution_record(step: &WorkflowStep, signal: NormalizedSignal) -> StepExecutionRecord {
    StepExecutionRecord::builder()
        .step_id(step.id.clone())
        .worker_signal(signal)
        .updated_artifacts(step.execution.created_artifacts.clone())
        .build()
}

fn evaluator_execution_record(
    worker_execution: &StepExecutionRecord,
    evaluator_signal: NormalizedSignal,
    evaluator_output: Option<OutputText>,
) -> StepExecutionRecord {
    StepExecutionRecord::builder()
        .step_id(worker_execution.step_id.clone())
        .worker_signal(worker_execution.worker_signal.clone())
        .evaluator_record(
            StepEvaluatorRecord::builder()
                .maybe_evaluator_signal(Some(evaluator_signal))
                .maybe_evaluator_output(evaluator_output)
                .build(),
        )
        .updated_artifacts(worker_execution.updated_artifacts.clone())
        .remediation_record(worker_execution.remediation_record.clone())
        .build()
}

/// Dispatches the configured quick-patch agent, then re-dispatches the failing reviewer.
///
/// On patch pass: restores the run cursor and pending worker, then dispatches the reviewer
/// via the normal `dispatch_request` path. The reviewer's completion is handled by the
/// standard command loop.
///
/// On patch fail or dispatch error: emits [`DeterministicOrchestratorEvent::Halted`] and
/// clears the run cursor.
async fn handle_delegate_fix(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    args: DelegateFixArgs,
) {
    let Some(reviewer_step) = workflow_step(state, &args.return_to_reviewer).cloned() else {
        emit_halted(&ports.event_tx, args.step_id);
        state.run_state.current_step_id = None;
        return;
    };

    tracing::info!(
        step_id = %args.step_id,
        return_to_reviewer = %args.return_to_reviewer,
        patch_agent = %args.patch_agent,
        attempt = args.attempt,
        "dispatching delegate-fix quick patch",
    );

    let patch_request = build_patch_dispatch_request(
        &args.patch_agent,
        &reviewer_step,
        args.failure_notes.as_ref(),
    );

    let patch_signal = dispatch_patch_agent_and_await(ports, &args.step_id, patch_request).await;

    if patch_signal != NormalizedSignal::Advance {
        emit_halted(&ports.event_tx, args.step_id);
        state.run_state.current_step_id = None;
        return;
    }

    restore_reviewer_state_and_dispatch(
        state,
        ports,
        ReviewerRestoreArgs::builder()
            .reviewer_step(reviewer_step)
            .return_to_reviewer(args.return_to_reviewer)
            .step_id(args.step_id)
            .build(),
    )
    .await;
}

fn try_mock_remediation_dispatch(request: &WorkflowDispatchRequest) -> Option<NormalizedSignal> {
    let agent_name = request.dispatch.worker_agent.clone()?;
    MOCK_REMEDIATION_DISPATCHES
        .try_with(|queue| {
            let mut queue = queue.borrow_mut();
            let expected = queue.pop_front()?;
            assert_eq!(
                request.step_id, expected.request_step_id,
                "mock remediation dispatch should target the expected step",
            );
            assert_eq!(
                agent_name, expected.agent_name,
                "mock remediation dispatch should target the expected agent",
            );
            Some(expected.signal)
        })
        .ok()
        .flatten()
}

/// Dispatches a quick-patch agent and awaits its completion signal.
///
/// Returns [`NormalizedSignal::Hold`] on dispatch errors or completion errors.
async fn dispatch_patch_agent_and_await(
    ports: &RuntimePorts,
    step_id: &WorkflowStepId,
    patch_request: WorkflowDispatchRequest,
) -> NormalizedSignal {
    if let Some(mock_signal) = try_mock_remediation_dispatch(&patch_request) {
        return mock_signal;
    }

    let dispatcher = remediation_dispatcher(ports);
    let Some(ticket) = dispatch_patch_ticket(&dispatcher, &patch_request, step_id).await else {
        return NormalizedSignal::Hold;
    };
    await_patch_signal(&dispatcher, ticket, step_id).await
}

fn remediation_dispatcher(ports: &RuntimePorts) -> DeterministicAgentDispatcher {
    match &ports.agent_feed_tx {
        Some(tx) => {
            DeterministicAgentDispatcher::new_with_feed(ports.dispatch_runtime.clone(), tx.clone())
        }
        None => DeterministicAgentDispatcher::new(ports.dispatch_runtime.clone()),
    }
}

async fn dispatch_patch_ticket(
    dispatcher: &DeterministicAgentDispatcher,
    patch_request: &WorkflowDispatchRequest,
    step_id: &WorkflowStepId,
) -> Option<AgentDispatchTicket> {
    match dispatcher.dispatch_worker_agent(patch_request).await {
        Ok(ticket) => Some(ticket),
        Err(err) => {
            tracing::warn!(
                step_id = %step_id,
                error = %err,
                "patch agent dispatch failed"
            );
            None
        }
    }
}

async fn await_patch_signal(
    dispatcher: &DeterministicAgentDispatcher,
    ticket: AgentDispatchTicket,
    step_id: &WorkflowStepId,
) -> NormalizedSignal {
    match dispatcher.await_agent_completion(ticket).await {
        Ok((signal, _)) => signal,
        Err(err) => {
            tracing::warn!(
                step_id = %step_id,
                error = %err,
                "patch agent completion failed"
            );
            NormalizedSignal::Hold
        }
    }
}

/// Bundled arguments for re-dispatching the reviewer after a successful patch.
#[derive(bon::Builder)]
struct ReviewerRestoreArgs {
    /// The reviewer step to re-dispatch.
    reviewer_step: WorkflowStep,
    /// Step ID of the reviewer (used to locate prior execution and restore cursor).
    return_to_reviewer: WorkflowStepId,
    /// Failing step ID used for halt events when restoration fails.
    step_id: WorkflowStepId,
}

/// Restores run cursor and pending-worker state, then re-dispatches the reviewer evaluator.
///
/// On missing prior worker execution: emits Halted and clears the cursor.
/// Finds the most-recent worker execution record for `return_to_reviewer` in prior steps.
///
/// Returns `None` if no record exists; the caller must handle the missing-record case.
fn find_prior_worker_execution(
    state: &DeterministicOrchestratorRunState,
    return_to_reviewer: &WorkflowStepId,
) -> Option<StepExecutionRecord> {
    state
        .run_state
        .prior_steps
        .iter()
        .rev()
        .find(|r| r.step_id == *return_to_reviewer)
        .cloned()
}

/// Reconstructs a bare worker `StepExecutionRecord` from a prior record.
///
/// Copies only `step_id`, `worker_signal`, and `updated_artifacts` - dropping any
/// evaluator or remediation data - so the evaluator sees a clean worker baseline.
fn rebuild_worker_execution_from_prior(prior: &StepExecutionRecord) -> StepExecutionRecord {
    StepExecutionRecord::builder()
        .step_id(prior.step_id.clone())
        .worker_signal(prior.worker_signal.clone())
        .updated_artifacts(prior.updated_artifacts.clone())
        .build()
}

async fn restore_reviewer_state_and_dispatch(
    state: &mut DeterministicOrchestratorRunState,
    ports: &RuntimePorts,
    args: ReviewerRestoreArgs,
) {
    let Some(prior_record) = find_prior_worker_execution(state, &args.return_to_reviewer) else {
        tracing::warn!(
            step_id = %args.return_to_reviewer,
            "no prior worker execution for reviewer step after patch - halting"
        );
        emit_halted(&ports.event_tx, args.step_id);
        state.run_state.current_step_id = None;
        return;
    };

    let worker_execution = rebuild_worker_execution_from_prior(&prior_record);

    let Some(_) = args.reviewer_step.dispatch.evaluator_agent.as_ref() else {
        tracing::warn!(
            failing_step_id = %args.step_id,
            reviewer_step_id = %args.return_to_reviewer,
            reviewer_kind = ?args.reviewer_step.kind,
            "delegate-fix reviewer restore cannot re-dispatch a single-pass step without an evaluator"
        );
        emit_halted(&ports.event_tx, args.step_id);
        state.run_state.current_step_id = None;
        return;
    };

    state.run_state.current_step_id = Some(args.return_to_reviewer.clone());
    state.pending_worker = Some(PendingStepExecution {
        execution: worker_execution.clone(),
        artifact_updates: vec![],
    });

    dispatch_request(
        ports,
        state.artifact_store.clone(),
        build_evaluator_dispatch_request(
            &args.reviewer_step,
            &worker_execution,
            state.progress.feature_context.clone(),
        ),
    )
    .await;
}
