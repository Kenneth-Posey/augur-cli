//! Phase 1 domain contracts for the deterministic orchestrator.

use augur_domain::domain::{
    AgentName, FilePath, IsPredicate, ModelName, OutputText, PassCriterion, PromptText,
    StringNewtype, WorkflowSignalValue, WorkflowStageId, WorkflowStepId, WorkflowThinkingDepth,
};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

/// Ordered workflow document parsed from workflow-like YAML input.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct WorkflowDocument {
    /// Declared workflow stages in source order.
    pub stages: Vec<WorkflowStage>,
}

impl WorkflowDocument {
    /// Returns declared stage identifiers in their source order.
    pub fn declared_stage_ids(&self) -> Vec<WorkflowStageId> {
        self.stages.iter().map(|stage| stage.id.clone()).collect()
    }
}

/// Ordered stage contract.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct WorkflowStage {
    /// Stable stage identifier.
    #[serde(alias = "stage_id")]
    pub id: WorkflowStageId,
    /// Stage steps in declared order.
    pub steps: Vec<WorkflowStep>,
}

/// Runtime step contract derived from the workflow document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowStep {
    /// Stable step identifier.
    pub id: WorkflowStepId,
    /// Lowered execution mode.
    pub kind: WorkflowStepKind,
    /// Worker and evaluator dispatch metadata for this step.
    pub dispatch: AgentDispatchSpec,
    /// Step-local artifact and lowered-member metadata.
    pub execution: WorkflowStepExecution,
    /// Pass and fail transition metadata.
    pub transition: WorkflowTransition,
}

impl<'de> serde::Deserialize<'de> for WorkflowStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawWorkflowStep::deserialize(deserializer)?;
        build_workflow_step(
            raw.id,
            raw.kind.into(),
            WorkflowStepParts::builder()
                .dispatch(raw.dispatch)
                .execution(raw.execution)
                .transition(raw.transition)
                .build(),
        )
        .map_err(D::Error::custom)
    }
}

/// Step execution modes supported by the deterministic workflow contract.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepKind {
    /// Step uses a worker agent followed by an evaluator gate.
    WorkerWithGate,
    /// Step uses one authoritative worker-only pass.
    #[serde(rename = "single_pass", alias = "single_agent")]
    SinglePass,
    /// Step contains members that are lowered into deterministic executable work.
    ParallelGroup,
    /// Structural lowering marker for a declared parallel-group member when the member omits an explicit executable step type.
    GroupMember,
}

impl WorkflowStepKind {
    fn yaml_name(&self) -> &'static str {
        match self {
            Self::WorkerWithGate => "worker_with_gate",
            Self::SinglePass => "single_pass",
            Self::ParallelGroup => "parallel_group",
            Self::GroupMember => "group_member",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ParsedWorkflowStepKind {
    WorkerWithGate,
    #[serde(rename = "single_pass", alias = "single_agent")]
    SinglePass,
    ParallelGroup,
}

impl From<ParsedWorkflowStepKind> for WorkflowStepKind {
    fn from(kind: ParsedWorkflowStepKind) -> Self {
        match kind {
            ParsedWorkflowStepKind::WorkerWithGate => Self::WorkerWithGate,
            ParsedWorkflowStepKind::SinglePass => Self::SinglePass,
            ParsedWorkflowStepKind::ParallelGroup => Self::ParallelGroup,
        }
    }
}

impl WorkflowStepKind {
    /// Returns `true` if this step kind can be executed by the runner.
    ///
    /// Returns a plain `bool` because this is a predicate method, not a domain value.
    pub(crate) fn is_executable(&self) -> IsPredicate {
        IsPredicate::from(matches!(self, Self::WorkerWithGate | Self::SinglePass))
    }

    /// Returns `true` if this step kind requires an evaluator pass after the worker.
    ///
    /// Returns a plain `bool` because this is a predicate method, not a domain value.
    pub(crate) fn requires_evaluator(&self) -> IsPredicate {
        IsPredicate::from(matches!(self, Self::WorkerWithGate))
    }
}

/// Returns the structural default step kind used only when a lowered group member omits an explicit `step_type`.
fn default_group_member_step_kind() -> WorkflowStepKind {
    WorkflowStepKind::GroupMember
}

/// Helper contract used only when deserializing lowered group members.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
struct RawWorkflowStep {
    /// Stable step identifier.
    #[serde(alias = "step_id")]
    id: WorkflowStepId,
    /// Canonical parsed execution mode.
    #[serde(alias = "step_type")]
    kind: ParsedWorkflowStepKind,
    /// Worker and evaluator dispatch metadata for this step.
    #[serde(flatten, default)]
    dispatch: AgentDispatchSpec,
    /// Step-local artifact and lowered-member metadata.
    #[serde(flatten, default)]
    execution: WorkflowStepExecution,
    /// Pass and fail transition metadata.
    #[serde(flatten, default)]
    transition: WorkflowTransition,
}

/// Helper contract used only when deserializing lowered group members.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
struct LoweredGroupMemberStep {
    /// Stable step identifier.
    #[serde(alias = "step_id")]
    id: WorkflowStepId,
    /// Execution mode preserved from the lowered member YAML.
    #[serde(alias = "step_type")]
    kind: Option<ParsedWorkflowStepKind>,
    /// Worker and evaluator dispatch metadata for this step.
    #[serde(flatten, default)]
    dispatch: AgentDispatchSpec,
    /// Step-local artifact and lowered-member metadata.
    #[serde(flatten, default)]
    execution: WorkflowStepExecution,
    /// Pass and fail transition metadata.
    #[serde(flatten, default)]
    transition: WorkflowTransition,
}

impl TryFrom<LoweredGroupMemberStep> for WorkflowStep {
    type Error = String;

    fn try_from(member: LoweredGroupMemberStep) -> Result<Self, Self::Error> {
        let kind = member
            .kind
            .map(WorkflowStepKind::from)
            .unwrap_or_else(default_group_member_step_kind);
        build_workflow_step(
            member.id,
            kind,
            WorkflowStepParts::builder()
                .dispatch(member.dispatch)
                .execution(member.execution)
                .transition(member.transition)
                .build(),
        )
    }
}

#[derive(bon::Builder)]
struct WorkflowStepParts {
    dispatch: AgentDispatchSpec,
    execution: WorkflowStepExecution,
    transition: WorkflowTransition,
}

/// Typed dispatch metadata for a workflow step's worker pass and optional evaluator pass.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct AgentDispatchSpec {
    /// Model identifier used for the step's dispatches.
    #[serde(default)]
    pub model: Option<ModelName>,
    /// Optional thinking-depth label from the workflow contract.
    #[serde(default)]
    pub thinking_depth: Option<WorkflowThinkingDepth>,
    /// Worker agent invoked for this step, when the step is executable.
    #[serde(default, alias = "worker_agent")]
    pub worker_agent: Option<AgentName>,
    /// Evaluator agent invoked after the worker pass, when present.
    #[serde(default, alias = "gate_agent")]
    pub evaluator_agent: Option<AgentName>,
    /// Optional prompt override for future request builders.
    #[serde(default)]
    pub prompt: Option<PromptText>,
}

/// Step-local execution metadata kept separate so `WorkflowStep` stays compact.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct WorkflowStepExecution {
    /// Artifacts that must exist before dispatch.
    #[serde(default)]
    pub expected_inputs: Vec<WorkflowArtifactRef>,
    /// Artifacts that the step updates or creates.
    #[serde(default)]
    pub created_artifacts: Vec<WorkflowArtifactRef>,
    /// Lowered group members in declared order.
    #[serde(default, deserialize_with = "deserialize_lowered_group_members")]
    pub members: Vec<WorkflowStep>,
    /// Conditions that must hold for the step to pass.
    #[serde(default)]
    pub pass_criteria: Vec<PassCriterion>,
    /// Conditions that cause the step to fail immediately.
    #[serde(default)]
    pub fail_criteria: Vec<String>,
}

/// Deserializes lowered group members while preserving any explicit member
/// `step_type` and defaulting only omitted member kinds within the `members`
/// collection.
fn deserialize_lowered_group_members<'de, D>(deserializer: D) -> Result<Vec<WorkflowStep>, D::Error>
where
    D: Deserializer<'de>,
{
    let members = Vec::<LoweredGroupMemberStep>::deserialize(deserializer)?;
    members
        .into_iter()
        .map(WorkflowStep::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(D::Error::custom)
}

/// Semantic reference to a workflow input or output artifact.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(transparent)]
pub struct WorkflowArtifactRef {
    /// Artifact path preserved from the workflow contract.
    pub path: FilePath,
}

/// Pass and fail transition metadata for a workflow step.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct WorkflowTransition {
    /// Transition applied when the step passes.
    #[serde(default)]
    pub on_pass: WorkflowPassTransition,
    /// Transition applied when the step fails.
    #[serde(default)]
    pub on_fail: WorkflowFailureTransition,
    /// Routing applied when the step emits `needs-revision`. When action is
    /// `Unspecified`, `NeedsRevision` falls through to `on_fail` (fail-closed).
    #[serde(default)]
    pub on_needs_revision: WorkflowFailureTransition,
}

/// Forward transition metadata for a passing step.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct WorkflowPassTransition {
    /// Next step declared by the workflow, if any.
    #[serde(alias = "next_step")]
    pub next_step_id: Option<WorkflowStepId>,
}

/// Failure transition metadata read from the workflow contract.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct WorkflowFailureTransition {
    /// Failure action selected by the workflow contract.
    #[serde(default)]
    pub action: WorkflowFailureAction,
    /// Optional next-step identifier used by continue-style fail paths.
    #[serde(default, alias = "next_step")]
    pub next_step_id: Option<WorkflowStepId>,
    /// Optional backward target identifier used by backtrack-style fail paths.
    #[serde(default, alias = "backward_step")]
    pub backward_step_id: Option<WorkflowStepId>,
    /// Quick-patch agent dispatched on FailureOrigin::Step failures.
    /// When present, the policy emits DelegateFix before falling through
    /// to backtrack or halt. When absent, existing behavior is unchanged.
    #[serde(default)]
    pub quick_patch_agent: Option<AgentName>,
}

/// Static failure actions declared on a workflow step.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowFailureAction {
    /// No failure action was declared; the runtime policy selects rerun, backtrack, or halt.
    #[default]
    Unspecified,
    /// Halt the workflow immediately.
    Halt,
    /// Re-run the current step.
    RerunCurrentStep,
    /// Jump backward to a prior executable step.
    Backtrack,
    /// Continue to an explicitly declared next step.
    ContinueToNextStep,
    /// Record a member failure and continue the enclosing group.
    RecordFailAndContinueGroup,
    /// Dispatch a remediation agent then retry the failing checkers.
    /// Also accepts the alias `"quick-patch-and-retry"` from YAML.
    #[serde(alias = "quick-patch-and-retry")]
    RemediateAndRetry,
}

impl WorkflowFailureAction {
    /// Returns `true` if failure handling should apply the declared YAML path directly
    /// instead of consulting a dynamic policy boundary.
    ///
    /// Returns a plain `bool` because this is a predicate method, not a domain value.
    ///
    /// Returns `false` for `Unspecified` (no explicit `on_fail` declared) and for
    /// `RemediateAndRetry` (patcher dispatch must be driven by the `DelegateFix` policy
    /// machinery, not by the declared-action path).  All other variants represent
    /// explicit declarations that bypass the policy.
    pub(crate) fn uses_declared_automatic_transition(&self) -> IsPredicate {
        IsPredicate::from(!matches!(self, Self::Unspecified | Self::RemediateAndRetry))
    }
}

fn build_workflow_step(
    id: WorkflowStepId,
    kind: WorkflowStepKind,
    parts: WorkflowStepParts,
) -> Result<WorkflowStep, String> {
    parts.dispatch.validate_for_step_kind(&id, &kind)?;

    Ok(WorkflowStep {
        id,
        kind,
        dispatch: parts.dispatch,
        execution: parts.execution,
        transition: parts.transition,
    })
}

impl AgentDispatchSpec {
    fn validate_for_step_kind(
        &self,
        step_id: &WorkflowStepId,
        step_kind: &WorkflowStepKind,
    ) -> Result<(), String> {
        match step_kind {
            WorkflowStepKind::WorkerWithGate => self.validate_worker_with_gate(step_id, step_kind),
            WorkflowStepKind::SinglePass => self.validate_single_pass(step_id, step_kind),
            WorkflowStepKind::ParallelGroup | WorkflowStepKind::GroupMember => Ok(()),
        }
    }

    fn validate_worker_with_gate(
        &self,
        step_id: &WorkflowStepId,
        step_kind: &WorkflowStepKind,
    ) -> Result<(), String> {
        self.require_field(
            RequiredField::builder()
                .step_id(step_id)
                .step_kind(step_kind)
                .field_name("model")
                .is_present(self.model.is_some())
                .build(),
        )?;
        self.require_field(
            RequiredField::builder()
                .step_id(step_id)
                .step_kind(step_kind)
                .field_name("thinking_depth")
                .is_present(self.thinking_depth.is_some())
                .build(),
        )?;
        self.require_field(
            RequiredField::builder()
                .step_id(step_id)
                .step_kind(step_kind)
                .field_name("worker_agent")
                .is_present(self.worker_agent.is_some())
                .build(),
        )?;
        self.require_field(
            RequiredField::builder()
                .step_id(step_id)
                .step_kind(step_kind)
                .field_name("gate_agent")
                .is_present(self.evaluator_agent.is_some())
                .build(),
        )
    }

    fn validate_single_pass(
        &self,
        step_id: &WorkflowStepId,
        step_kind: &WorkflowStepKind,
    ) -> Result<(), String> {
        self.require_field(
            RequiredField::builder()
                .step_id(step_id)
                .step_kind(step_kind)
                .field_name("model")
                .is_present(self.model.is_some())
                .build(),
        )?;
        self.require_field(
            RequiredField::builder()
                .step_id(step_id)
                .step_kind(step_kind)
                .field_name("thinking_depth")
                .is_present(self.thinking_depth.is_some())
                .build(),
        )?;
        self.require_field(
            RequiredField::builder()
                .step_id(step_id)
                .step_kind(step_kind)
                .field_name("worker_agent")
                .is_present(self.worker_agent.is_some())
                .build(),
        )
    }

    fn require_field(&self, required: RequiredField<'_>) -> Result<(), String> {
        if required.is_present {
            return Ok(());
        }

        Err(format!(
            "workflow step `{step_id}` with step_type `{}` is missing required field `{field_name}`",
            required.step_kind.yaml_name(),
            step_id = required.step_id,
            field_name = required.field_name,
        ))
    }
}

#[derive(bon::Builder)]
struct RequiredField<'a> {
    step_id: &'a WorkflowStepId,
    step_kind: &'a WorkflowStepKind,
    field_name: &'a str,
    is_present: bool,
}

/// Dynamic failure choice returned by later policy and transition logic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureDecision {
    /// Re-run the current step.
    RerunCurrentStep,
    /// Move backward to a previously executed step.
    BacktrackTo {
        /// Target step identifier selected for backtracking.
        step_id: WorkflowStepId,
    },
    /// Halt the workflow without advancing.
    Halt,
    /// Dispatch a quick-patch agent then re-run the reviewer that failed.
    DelegateFix {
        /// Quick-patch agent to dispatch with failure notes.
        patch_agent: AgentName,
        /// Reviewer step to re-run after the patch agent completes.
        return_to_reviewer: WorkflowStepId,
        /// Attempt number (1 or 2). After 2, policy falls through to BacktrackTo.
        attempt: u8,
    },
}

/// Semantic origin of the failure that the runtime is currently resolving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureOrigin {
    /// Infrastructure around dispatch or completion failed.
    Infrastructure,
    /// The step completed normally but did not produce a passing result.
    Step,
}

/// Evaluator-related results for a workflow step execution attempt.
#[derive(Clone, Debug, Default, PartialEq, Eq, bon::Builder)]
pub struct StepEvaluatorRecord {
    /// Normalized evaluator result when the step has an evaluator pass.
    pub evaluator_signal: Option<NormalizedSignal>,
    /// Full evaluator output text when the evaluator emitted Hold.
    /// Empty when the evaluator passed or the step has no evaluator.
    pub evaluator_output: Option<OutputText>,
}

/// Signal and dispatch identity for a single parallel group member.
#[derive(Clone, Debug, PartialEq, Eq, bon::Builder)]
pub struct GroupMemberResult {
    /// Step identifier of the parallel group member.
    pub step_id: WorkflowStepId,
    /// Agent dispatched for this member execution attempt.
    pub agent_name: AgentName,
    /// Normalized signal emitted by this member.
    pub signal: NormalizedSignal,
    /// Failure decision applied to this member after the attempt, if any.
    pub failure_decision: Option<FailureDecision>,
}

/// Remediation tracking record for a workflow step execution attempt.
///
/// Bundles member-level outcomes, cycle-guard state, and the resolved failure
/// decision so that `StepExecutionRecord` stays within the five-field limit.
#[derive(Clone, Debug, Default, PartialEq, Eq, bon::Builder)]
pub struct StepRemediationRecord {
    /// Individual member outcomes for parallel_group steps.
    /// Empty for all other step kinds.
    #[builder(default)]
    pub member_results: Vec<GroupMemberResult>,
    /// Set to `true` after a remediation pass has been attempted for this step.
    /// Prevents an infinite fix→validate→fail cycle.
    #[builder(default)]
    pub remediation_attempted: IsPredicate,
    /// Failure decision applied after the attempt, if any.
    pub failure_decision: Option<FailureDecision>,
}

/// Per-step immutable execution history used for rerun and backtrack logic.
#[derive(Clone, Debug, PartialEq, Eq, bon::Builder)]
pub struct StepExecutionRecord {
    /// Executed step identifier.
    pub step_id: WorkflowStepId,
    /// Normalized worker result for the attempt.
    pub worker_signal: NormalizedSignal,
    /// Evaluator signal and output captured during the evaluator pass, if any.
    #[builder(default)]
    pub evaluator_record: StepEvaluatorRecord,
    /// Artifacts updated during the attempt.
    pub updated_artifacts: Vec<WorkflowArtifactRef>,
    /// Remediation tracking for this step, including the resolved failure decision.
    #[builder(default)]
    pub remediation_record: StepRemediationRecord,
}

/// Pending failure context held until later transition logic resolves it.
#[derive(Clone, Debug, PartialEq, Eq, bon::Builder)]
pub struct PendingFailureContext {
    /// Step that produced the pending failure.
    pub step_id: WorkflowStepId,
    /// Last normalized signal observed for the failing step.
    pub last_signal: NormalizedSignal,
    /// Semantic origin of the failing result routed into the policy boundary.
    pub origin: FailureOrigin,
    /// Full output text from the reviewer that produced this failure.
    /// Passed verbatim to the quick-patch agent prompt.
    pub failure_notes: Option<OutputText>,
}

/// Actor-owned workflow run state used by later orchestration phases.
#[derive(Clone, Debug, Default, bon::Builder)]
pub struct WorkflowRunState {
    /// Current workflow step cursor, if the run has started.
    pub current_step_id: Option<WorkflowStepId>,
    /// Previously executed steps that future backtrack logic may target.
    pub prior_steps: Vec<StepExecutionRecord>,
    /// Pending failure metadata awaiting a typed decision.
    pub pending_failure: Option<PendingFailureContext>,
}

impl WorkflowRunState {
    /// Returns prior executed step identifiers in backward-ready order.
    pub fn backtrack_ready_step_ids(&self) -> Vec<WorkflowStepId> {
        self.prior_steps
            .iter()
            .map(|record| record.step_id.clone())
            .collect()
    }
}

/// Minimal fail-closed signal contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NormalizedSignal {
    /// Execution may advance.
    Advance,
    /// Step output requires revision before advancing; distinct from a hard failure.
    NeedsRevision,
    /// Execution must not advance.
    Hold,
}

impl NormalizedSignal {
    /// Normalizes a raw evaluator signal using an exact, fail-closed pass check.
    pub fn from_raw(raw: &WorkflowSignalValue) -> Self {
        match raw.as_str() {
            "pass" => Self::Advance,
            "needs-revision" => Self::NeedsRevision,
            _ => Self::Hold,
        }
    }
}

/// Runtime events emitted by the deterministic orchestrator actor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeterministicOrchestratorEvent {
    /// Workflow execution started.
    Started {
        /// First step selected for execution, if any.
        first_step_id: Option<WorkflowStepId>,
    },
    /// A workflow step reported progress.
    StepProgressed {
        /// Step that produced the progress event.
        step_id: WorkflowStepId,
        /// Normalized signal recorded for that progress update.
        signal: NormalizedSignal,
        /// Name of the worker or evaluator agent that produced this signal.
        agent_name: Option<String>,
    },
    /// The runtime scheduled a rerun of the current step.
    RerunScheduled {
        /// Step selected for rerun.
        step_id: WorkflowStepId,
    },
    /// The runtime moved backward to a prior step.
    Backtracked {
        /// Step that was left.
        from_step_id: WorkflowStepId,
        /// Step selected as the new cursor.
        to_step_id: WorkflowStepId,
    },
    /// The runtime halted after a terminal failure.
    Halted {
        /// Step at which the workflow halted.
        step_id: WorkflowStepId,
    },
    /// Workflow execution completed.
    Completed,
}
