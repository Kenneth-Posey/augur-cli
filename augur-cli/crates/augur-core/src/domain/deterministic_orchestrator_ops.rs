//! Phase 2 pure-logic compile-targets for the deterministic orchestrator.

use crate::domain::deterministic_orchestrator::{
    AgentDispatchSpec, FailureDecision, NormalizedSignal, StepExecutionRecord, WorkflowArtifactRef,
    WorkflowDocument, WorkflowFailureAction, WorkflowRunState, WorkflowStep,
};
use augur_domain::domain::{
    AgentName, Count, FeatureContext, FeatureSlug, FilePath, IsPredicate, OutputText,
    PassCriterion, StringNewtype, WorkflowSignalValue, WorkflowStepId,
};
use std::collections::BTreeMap;

/// Pure path-policy result for local workflow source selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalWorkflowSourceAction {
    /// Reuse the existing `.github/local/plan_execution.yml` file.
    UseExistingLocalWorkflow,
    /// Seed `.github/local/plan_execution.yml` from `.github/plan_execution.yml`.
    SeedLocalWorkflowFromCanonical,
}

/// Semantic presence marker for the local workflow file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalWorkflowPresence {
    /// `.github/local/plan_execution.yml` already exists.
    Present,
    /// `.github/local/plan_execution.yml` is absent and may need seeding.
    Absent,
}

/// Deterministic execution index derived from the parsed workflow contract.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepIndex {
    /// Executable step identifiers in runtime order after any group lowering.
    pub ordered_executable_step_ids: Vec<WorkflowStepId>,
    /// Every declared step id mapped to the first executable runtime step reached when entering it.
    pub first_executable_by_declared_step_id: BTreeMap<WorkflowStepId, WorkflowStepId>,
    executable_position_by_step_id: BTreeMap<WorkflowStepId, usize>,
    workflow_step_by_id: BTreeMap<WorkflowStepId, WorkflowStep>,
}

/// Indexed execution history used by rerun and backtrack helpers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExecutedStepIndex {
    attempt_count_by_step_id: BTreeMap<WorkflowStepId, usize>,
    last_execution_order_by_step_id: BTreeMap<WorkflowStepId, usize>,
    step_id_by_execution_order: BTreeMap<usize, WorkflowStepId>,
    next_execution_order: usize,
}

/// Distinguishes worker and evaluator dispatch planning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DispatchRequestKind {
    /// First dispatch for a workflow step.
    Worker,
    /// Evaluator dispatch that follows worker completion.
    Evaluator,
}

/// Pure dispatch-planning output for later adapter execution.
#[derive(Clone, Debug, Default, PartialEq, Eq, bon::Builder)]
pub struct WorkflowDispatchArtifacts {
    /// Step inputs required before dispatch.
    pub expected_inputs: Vec<WorkflowArtifactRef>,
    /// Step outputs updated by this execution.
    pub created_artifacts: Vec<WorkflowArtifactRef>,
    /// Criteria forwarded to the dispatched agent for pass/fail evaluation.
    pub pass_criteria: Vec<PassCriterion>,
    /// Optional free-form feature context forwarded to the worker prompt.
    pub feature_context: Option<FeatureContext>,
}

/// Pure dispatch-planning output for later adapter execution.
#[derive(Clone, Debug, PartialEq, Eq, bon::Builder)]
pub struct WorkflowDispatchRequest {
    /// Distinguishes the worker and evaluator passes.
    pub kind: DispatchRequestKind,
    /// Step the request belongs to.
    pub step_id: WorkflowStepId,
    /// Typed dispatch metadata copied from the workflow contract.
    pub dispatch: AgentDispatchSpec,
    /// Step input and output artifact metadata preserved from the contract.
    pub artifacts: WorkflowDispatchArtifacts,
    /// Prior worker execution attached only to evaluator requests.
    pub prior_execution: Option<StepExecutionRecord>,
}

/// Pure pass-transition result derived from the current workflow contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PassTransitionResolution {
    /// Advance to the explicitly declared next step.
    AdvanceTo(WorkflowStepId),
    /// Complete the workflow because no next step is declared.
    Complete,
    /// Hold the current step because the normalized signal did not advance.
    StayOnCurrentStep,
}

/// Pure failure-transition result derived from typed decisions and workflow contracts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureTransitionResolution {
    /// Re-run the current step.
    RerunCurrentStep,
    /// Jump backward to a validated prior step.
    BacktrackTo(WorkflowStepId),
    /// Continue to the explicitly declared next step.
    ContinueToNextStep(WorkflowStepId),
    /// Halt execution without advancing.
    Halt,
    /// Dispatch a patch agent, then re-run the failing reviewer.
    DelegateFix {
        /// Quick-patch agent to dispatch.
        patch_agent: AgentName,
        /// Reviewer step to re-run after the patch completes.
        return_to_reviewer: WorkflowStepId,
        /// Step-failure attempt number forwarded for logging and future cap enforcement.
        attempt: u8,
        /// Full reviewer output text passed to the patch agent prompt.
        failure_notes: Option<OutputText>,
    },
}

/// Bundled read-only context for backtrack target validation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BacktrackValidationCtx<'a> {
    /// Runtime-derived executable step ordering for the current workflow.
    pub step_index: &'a StepIndex,
    /// Indexed executed-step history used for rerun and backtrack checks.
    pub executed_steps: &'a ExecutedStepIndex,
    /// Run state containing the current cursor and prior executed steps.
    pub run_state: &'a WorkflowRunState,
}

/// Semantic validation result for backtrack target checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BacktrackTargetValidation {
    /// The candidate step is executable, prior to the current step, and already executed.
    Valid,
    /// The candidate step fails one or more backtrack validation requirements.
    Invalid,
}

/// Shared read-only inputs for failure-transition helpers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FailureTransitionContext<'a> {
    /// Runtime-derived step index for validation.
    pub step_index: &'a StepIndex,
    /// Indexed executed-step history used for rerun and backtrack checks.
    pub executed_steps: &'a ExecutedStepIndex,
    /// Run state containing the current cursor and prior executed steps.
    pub run_state: &'a WorkflowRunState,
}

/// Decides whether adapters should reuse or seed the local workflow file.
///
/// Parameters:
/// - `local_workflow_presence`: semantic presence marker for
///   `.github/local/plan_execution.yml`.
///
/// Returns:
/// - [`LocalWorkflowSourceAction`]: a pure path-policy decision that never
///   touches the filesystem.
///
/// Side effects:
/// - None.
///
/// Invariants:
/// - Existing local workflow files always take precedence over canonical seeding.
pub fn decide_local_workflow_source_action(
    local_workflow_presence: LocalWorkflowPresence,
) -> LocalWorkflowSourceAction {
    match local_workflow_presence {
        LocalWorkflowPresence::Present => LocalWorkflowSourceAction::UseExistingLocalWorkflow,
        LocalWorkflowPresence::Absent => LocalWorkflowSourceAction::SeedLocalWorkflowFromCanonical,
    }
}

/// Builds the deterministic executable-step order from the parsed workflow contract.
///
/// Parameters:
/// - `document`: parsed workflow document whose declared stage and step order is
///   authoritative.
///
/// Returns:
/// - [`StepIndex`]: executable step ids in runtime order, with
///   `parallel_group` members lowered in declared member order.
///
/// Side effects:
/// - None.
///
/// Invariants:
/// - No hardcoded stage or step ids are consulted.
pub fn build_step_index(document: &WorkflowDocument) -> StepIndex {
    let mut builder = StepIndexBuilder::default();

    for stage in &document.stages {
        for step in &stage.steps {
            builder.append_step(step);
        }
    }

    builder.finish()
}

/// Builds an indexed executed-step history from the current run-state records.
pub(crate) fn build_executed_step_index(run_state: &WorkflowRunState) -> ExecutedStepIndex {
    let mut executed_steps = ExecutedStepIndex::default();

    for record in &run_state.prior_steps {
        executed_steps.record_execution(&record.step_id);
    }

    executed_steps
}

/// Derives a lowercase-hyphenated feature slug from a free-form request string.
///
/// Takes the first 5 non-empty, ASCII-alphanumeric words (split on whitespace
/// and punctuation), joins them with `-`, and lowercases the result.
/// Falls back to `"feature"` when no usable words are found.
///
/// Inputs:
/// - `request_text`: raw feature request or user message to derive a slug from.
///
/// Returns:
/// - A non-empty hyphen-joined slug string, always lowercase.
///
/// Side effects:
/// - None.
pub fn derive_feature_slug(request_text: &FeatureContext) -> FeatureSlug {
    let words: Vec<String> = request_text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .take(5)
        .map(|s| s.to_lowercase())
        .collect();
    if words.is_empty() {
        FeatureSlug::from("feature")
    } else {
        FeatureSlug::from(words.join("-"))
    }
}

/// Bundled extras for `build_dispatch_request` to stay within the 3-parameter limit.
struct DispatchOptions {
    /// Prior worker execution, attached only to evaluator dispatch requests.
    prior_execution: Option<StepExecutionRecord>,
    /// Optional free-form feature context forwarded into the worker prompt.
    feature_context: Option<FeatureContext>,
}

/// Builds the worker dispatch request for the current workflow step.
///
/// Parameters:
/// - `step`: executable workflow step whose typed dispatch and artifact metadata
///   should be preserved.
/// - `feature_context`: optional free-form feature context forwarded to the
///   worker prompt alongside step metadata.
///
/// Returns:
/// - [`WorkflowDispatchRequest`]: a worker-marked request for later adapter execution.
///
/// Side effects:
/// - None.
pub(crate) fn build_worker_dispatch_request(
    step: &WorkflowStep,
    feature_context: Option<FeatureContext>,
) -> WorkflowDispatchRequest {
    build_dispatch_request(
        step,
        DispatchRequestKind::Worker,
        DispatchOptions {
            prior_execution: None,
            feature_context,
        },
    )
}

/// Builds the evaluator dispatch request for the current workflow step.
///
/// Parameters:
/// - `step`: executable workflow step whose typed dispatch and artifact metadata
///   should be preserved.
/// - `worker_execution`: typed execution record from the worker pass that the
///   evaluator must inspect.
/// - `feature_context`: optional free-form feature context forwarded to the
///   evaluator prompt alongside step metadata.
///
/// Returns:
/// - [`WorkflowDispatchRequest`]: an evaluator-marked request that carries the
///   prior worker execution.
///
/// Side effects:
/// - None.
pub(crate) fn build_evaluator_dispatch_request(
    step: &WorkflowStep,
    worker_execution: &StepExecutionRecord,
    feature_context: Option<FeatureContext>,
) -> WorkflowDispatchRequest {
    build_dispatch_request(
        step,
        DispatchRequestKind::Evaluator,
        DispatchOptions {
            prior_execution: Some(worker_execution.clone()),
            feature_context,
        },
    )
}

/// Builds the quick-patch agent dispatch request for a failing reviewer step.
///
/// Parameters:
/// - `patch_agent`: quick-patch agent to dispatch.
/// - `reviewer_step`: the workflow step whose reviewer failed; its artifacts are forwarded.
/// - `failure_notes`: full reviewer output passed as feature context to the patch agent.
///
/// Returns:
/// - [`WorkflowDispatchRequest`]: a worker-marked request for the patch agent.
///
/// Side effects:
/// - None.
pub fn build_patch_dispatch_request(
    patch_agent: &AgentName,
    reviewer_step: &WorkflowStep,
    failure_notes: Option<&OutputText>,
) -> WorkflowDispatchRequest {
    let feature_context = failure_notes.map(|n| FeatureContext::from(n.to_string()));
    WorkflowDispatchRequest::builder()
        .kind(DispatchRequestKind::Worker)
        .step_id(reviewer_step.id.clone())
        .dispatch(AgentDispatchSpec {
            worker_agent: Some(patch_agent.clone()),
            ..Default::default()
        })
        .artifacts(
            WorkflowDispatchArtifacts::builder()
                .expected_inputs(vec![])
                .created_artifacts(reviewer_step.execution.created_artifacts.clone())
                .pass_criteria(vec![])
                .maybe_feature_context(feature_context)
                .build(),
        )
        .build()
}

/// Normalizes a raw agent signal using the deterministic fail-closed rule.
///
/// Parameters:
/// - `raw_signal`: raw signal value emitted by a worker or evaluator pass.
///
/// Returns:
/// - [`NormalizedSignal`]: [`NormalizedSignal::Advance`] for the exact `"pass"` value,
///   [`NormalizedSignal::NeedsRevision`] for `"needs-revision"`,
///   or [`NormalizedSignal::Hold`] for any other value.
///
/// Side effects:
/// - None.
pub fn normalize_agent_signal(raw_signal: &WorkflowSignalValue) -> NormalizedSignal {
    NormalizedSignal::from_raw(raw_signal)
}

/// Resolves the next action after a pass-path signal is observed.
///
/// Parameters:
/// - `step`: current workflow step whose declared `on_pass` transition is authoritative.
/// - `signal`: normalized signal emitted by the evaluator pass or authoritative single-pass execution.
///
/// Returns:
/// - [`PassTransitionResolution`]: stay when the signal does not advance,
///   advance only to the declared next step, or complete when no next step exists.
///
/// Side effects:
/// - None.
pub fn resolve_pass_transition(
    step: &WorkflowStep,
    signal: &NormalizedSignal,
) -> PassTransitionResolution {
    if signal != &NormalizedSignal::Advance {
        return PassTransitionResolution::StayOnCurrentStep;
    }

    match &step.transition.on_pass.next_step_id {
        Some(next_step_id) if next_step_id.as_str() == "RUN_COMPLETE" => {
            PassTransitionResolution::Complete
        }
        Some(next_step_id) => PassTransitionResolution::AdvanceTo(next_step_id.clone()),
        None => PassTransitionResolution::Complete,
    }
}

/// Resolves the failure-path transition from typed policy input and workflow contracts.
///
/// Parameters:
/// - `step`: current workflow step whose declared `on_fail` transition provides
///   the fail-closed default behavior.
/// - `decision`: optional typed failure decision from a later policy boundary.
/// - `context`: runtime-derived step index and read-only run state used to
///   validate backtrack targets.
///
/// Returns:
/// - [`FailureTransitionResolution`]: rerun, validated backtrack, declared
///   continue target, or halt.
///
/// Side effects:
/// - None.
pub(crate) fn resolve_failure_transition(
    step: &WorkflowStep,
    decision: Option<&FailureDecision>,
    context: FailureTransitionContext<'_>,
) -> FailureTransitionResolution {
    match decision {
        Some(FailureDecision::RerunCurrentStep) => FailureTransitionResolution::RerunCurrentStep,
        Some(FailureDecision::BacktrackTo { step_id }) => {
            resolve_backtrack_transition(&context, step_id)
        }
        Some(FailureDecision::Halt) => FailureTransitionResolution::Halt,
        Some(FailureDecision::DelegateFix {
            patch_agent,
            return_to_reviewer,
            attempt,
        }) => {
            let failure_notes = context
                .run_state
                .pending_failure
                .as_ref()
                .and_then(|pf| pf.failure_notes.clone());
            FailureTransitionResolution::DelegateFix {
                patch_agent: patch_agent.clone(),
                return_to_reviewer: return_to_reviewer.clone(),
                attempt: *attempt,
                failure_notes,
            }
        }
        None => resolve_declared_failure_transition(step, context),
    }
}

/// Validates whether a backtrack target is executable, known, and strictly prior.
///
/// Parameters:
/// - `ctx`: bundled step index, executed-step history, and run state needed for
///   validation.
/// - `target_step_id`: candidate step to revisit after a failure.
///
/// Returns:
/// - [`BacktrackTargetValidation::Valid`] when the target exists in the runtime
///   index, appears before the current step, and has already been executed.
/// - [`BacktrackTargetValidation::Invalid`] otherwise.
///
/// Side effects:
/// - None.
pub(crate) fn validate_backtrack_target(
    ctx: &BacktrackValidationCtx<'_>,
    target_step_id: &WorkflowStepId,
) -> BacktrackTargetValidation {
    let Some(current_step_id) = &ctx.run_state.current_step_id else {
        return BacktrackTargetValidation::Invalid;
    };

    let Some(current_position) = ctx.step_index.executable_position(current_step_id) else {
        return BacktrackTargetValidation::Invalid;
    };
    let Some(target_position) = ctx.step_index.executable_position(target_step_id) else {
        return BacktrackTargetValidation::Invalid;
    };

    if target_position >= current_position {
        return BacktrackTargetValidation::Invalid;
    }

    if ctx.executed_steps.was_executed(target_step_id).0 {
        BacktrackTargetValidation::Valid
    } else {
        BacktrackTargetValidation::Invalid
    }
}

/// Builds either a worker or evaluator dispatch request from a workflow step.
fn build_dispatch_request(
    step: &WorkflowStep,
    kind: DispatchRequestKind,
    opts: DispatchOptions,
) -> WorkflowDispatchRequest {
    WorkflowDispatchRequest::builder()
        .kind(kind)
        .step_id(step.id.clone())
        .dispatch(step.dispatch.clone())
        .artifacts(
            WorkflowDispatchArtifacts::builder()
                .expected_inputs(step.execution.expected_inputs.clone())
                .created_artifacts(step.execution.created_artifacts.clone())
                .pass_criteria(step.execution.pass_criteria.clone())
                .maybe_feature_context(opts.feature_context)
                .build(),
        )
        .maybe_prior_execution(opts.prior_execution)
        .build()
}

/// Resolves a backtrack request in a fail-closed way.
fn resolve_backtrack_transition(
    context: &FailureTransitionContext<'_>,
    step_id: &WorkflowStepId,
) -> FailureTransitionResolution {
    let ctx = BacktrackValidationCtx {
        step_index: context.step_index,
        executed_steps: context.executed_steps,
        run_state: context.run_state,
    };
    if validate_backtrack_target(&ctx, step_id) == BacktrackTargetValidation::Valid {
        FailureTransitionResolution::BacktrackTo(step_id.clone())
    } else {
        FailureTransitionResolution::Halt
    }
}

/// Resolves the default workflow-declared failure action when no policy override exists.
fn resolve_declared_failure_transition(
    step: &WorkflowStep,
    context: FailureTransitionContext<'_>,
) -> FailureTransitionResolution {
    match step.transition.on_fail.action {
        WorkflowFailureAction::Unspecified => FailureTransitionResolution::Halt,
        WorkflowFailureAction::Halt => FailureTransitionResolution::Halt,
        WorkflowFailureAction::RerunCurrentStep => FailureTransitionResolution::RerunCurrentStep,
        WorkflowFailureAction::Backtrack => resolve_declared_backtrack(step, context),
        WorkflowFailureAction::ContinueToNextStep => resolve_declared_next_step(step),
        WorkflowFailureAction::RecordFailAndContinueGroup => {
            resolve_group_continuation(step, context)
        }
        // Intentional: fail-closed. Quick-patch dispatch is only via DelegateFix policy, not from declared action.
        WorkflowFailureAction::RemediateAndRetry => FailureTransitionResolution::Halt,
    }
}

fn resolve_declared_backtrack(
    step: &WorkflowStep,
    context: FailureTransitionContext<'_>,
) -> FailureTransitionResolution {
    step.transition
        .on_fail
        .backward_step_id
        .as_ref()
        .map_or(FailureTransitionResolution::Halt, |step_id| {
            resolve_backtrack_transition(&context, step_id)
        })
}

fn resolve_declared_next_step(step: &WorkflowStep) -> FailureTransitionResolution {
    step.transition.on_fail.next_step_id.clone().map_or(
        FailureTransitionResolution::Halt,
        FailureTransitionResolution::ContinueToNextStep,
    )
}

fn resolve_group_continuation(
    step: &WorkflowStep,
    context: FailureTransitionContext<'_>,
) -> FailureTransitionResolution {
    context.step_index.next_executable_step_id(&step.id).map_or(
        FailureTransitionResolution::Halt,
        FailureTransitionResolution::ContinueToNextStep,
    )
}

/// Replaces `<feature-slug>` in every artifact path in a vec in place.
fn apply_slug_to_artifact_vec(artifacts: &mut [WorkflowArtifactRef], slug: &FeatureSlug) {
    for artifact in artifacts.iter_mut() {
        let new_path = artifact
            .path
            .as_str()
            .replace("<feature-slug>", slug.as_str());
        artifact.path = FilePath::from(new_path);
    }
}

impl StepIndex {
    /// Replaces `<feature-slug>` placeholders in all step artifact paths.
    ///
    /// Inputs:
    /// - `slug`: derived feature slug to substitute in place of `<feature-slug>`.
    ///
    /// Side effects:
    /// - Mutates `expected_inputs` and `created_artifacts` paths in every stored step.
    pub(crate) fn apply_slug(&mut self, slug: &FeatureSlug) {
        for step in self.workflow_step_by_id.values_mut() {
            apply_slug_to_artifact_vec(&mut step.execution.expected_inputs, slug);
            apply_slug_to_artifact_vec(&mut step.execution.created_artifacts, slug);
        }
    }

    /// Resolves a declared step ID to the ID of its first executable member
    /// step, enabling transition targets to skip over non-executable container
    /// steps.  Returns `None` if the declared ID is not present in the index.
    pub(crate) fn resolve_transition_target_step_id(
        &self,
        target_step_id: &WorkflowStepId,
    ) -> Option<WorkflowStepId> {
        self.first_executable_by_declared_step_id
            .get(target_step_id)
            .cloned()
    }

    /// Returns a reference to the [`WorkflowStep`] with the given ID, or
    /// `None` if no such step exists in the index.
    pub(crate) fn workflow_step(&self, step_id: &WorkflowStepId) -> Option<&WorkflowStep> {
        self.workflow_step_by_id.get(step_id)
    }

    /// Returns the zero-based position of `step_id` within the ordered
    /// executable sequence, or `None` if the step is not executable.
    pub(crate) fn executable_position(&self, step_id: &WorkflowStepId) -> Option<Count> {
        self.executable_position_by_step_id
            .get(step_id)
            .copied()
            .map(Count::from)
    }

    fn next_executable_step_id(&self, step_id: &WorkflowStepId) -> Option<WorkflowStepId> {
        let current_position = self.executable_position(step_id)?;
        self.ordered_executable_step_ids
            .get((*current_position) + 1)
            .cloned()
    }
}

impl ExecutedStepIndex {
    /// Records that `step_id` was executed, incrementing its attempt count and
    /// assigning a new execution-order slot.  If the step was previously
    /// recorded its old order entry is replaced so the most-recent-first
    /// iterator always reflects the latest execution.
    pub(crate) fn record_execution(&mut self, step_id: &WorkflowStepId) {
        if let Some(previous_order) = self
            .last_execution_order_by_step_id
            .insert(step_id.clone(), self.next_execution_order)
        {
            self.step_id_by_execution_order.remove(&previous_order);
        }

        self.step_id_by_execution_order
            .insert(self.next_execution_order, step_id.clone());
        self.next_execution_order += 1;
        *self
            .attempt_count_by_step_id
            .entry(step_id.clone())
            .or_default() += 1;
    }

    /// Returns the number of times `step_id` has been executed; returns `0`
    /// if the step has never been recorded.
    pub(crate) fn attempt_count(&self, step_id: &WorkflowStepId) -> Count {
        Count::from(
            self.attempt_count_by_step_id
                .get(step_id)
                .copied()
                .unwrap_or_default(),
        )
    }

    /// Returns `true` if `step_id` has been executed at least once.
    pub(crate) fn was_executed(&self, step_id: &WorkflowStepId) -> IsPredicate {
        IsPredicate::from(self.last_execution_order_by_step_id.contains_key(step_id))
    }

    /// Returns an iterator over all executed step IDs in reverse execution
    /// order, yielding the most recently executed step first.
    pub(crate) fn most_recent_step_ids(&self) -> impl Iterator<Item = &WorkflowStepId> {
        self.step_id_by_execution_order.values().rev()
    }
}

#[derive(Default)]
struct StepIndexBuilder {
    ordered_executable_step_ids: Vec<WorkflowStepId>,
    first_executable_by_declared_step_id: BTreeMap<WorkflowStepId, WorkflowStepId>,
    executable_position_by_step_id: BTreeMap<WorkflowStepId, usize>,
    workflow_step_by_id: BTreeMap<WorkflowStepId, WorkflowStep>,
}

impl StepIndexBuilder {
    fn append_step(&mut self, step: &WorkflowStep) -> Option<WorkflowStepId> {
        self.workflow_step_by_id
            .insert(step.id.clone(), step.clone());

        let first_executable_step_id = if step.kind.is_executable().0 {
            let position = self.ordered_executable_step_ids.len();
            self.ordered_executable_step_ids.push(step.id.clone());
            self.executable_position_by_step_id
                .insert(step.id.clone(), position);
            Some(step.id.clone())
        } else {
            let mut first_executable_step_id = None;

            for member in &step.execution.members {
                let member_first_executable_step_id = self.append_step(member);
                if first_executable_step_id.is_none() {
                    first_executable_step_id = member_first_executable_step_id;
                }
            }

            first_executable_step_id
        };

        if let Some(first_executable_step_id) = first_executable_step_id.clone() {
            self.first_executable_by_declared_step_id
                .insert(step.id.clone(), first_executable_step_id);
        }

        first_executable_step_id
    }

    fn finish(self) -> StepIndex {
        StepIndex {
            ordered_executable_step_ids: self.ordered_executable_step_ids,
            first_executable_by_declared_step_id: self.first_executable_by_declared_step_id,
            executable_position_by_step_id: self.executable_position_by_step_id,
            workflow_step_by_id: self.workflow_step_by_id,
        }
    }
}
