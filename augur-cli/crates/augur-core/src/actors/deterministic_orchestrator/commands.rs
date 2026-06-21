//! Commands sent to the deterministic orchestrator actor.

use super::artifact_store::ArtifactUpdate;
use crate::domain::deterministic_orchestrator::FailureDecision;
use crate::domain::deterministic_orchestrator::NormalizedSignal;
use crate::domain::deterministic_orchestrator_ops::DispatchRequestKind;
use augur_domain::domain::{OutputText, WorkflowStepId};

/// Commands accepted by the deterministic orchestrator runtime actor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DeterministicOrchestratorCmd {
    /// Begin runtime execution from the repo-local workflow file.
    Start {
        /// Combined user message + file attachment content, if provided.
        /// When `None`, the pipeline relies on conversation history as context.
        feature_context: Option<String>,
        /// User-supplied feature slug override, if provided via `--slug`.
        /// When `None`, the slug is derived from `feature_context` at runtime.
        feature_slug: Option<String>,
        /// When `true`, skip steps whose output artifacts already exist on disk.
        resume: bool,
    },
    /// Record worker completion for the current workflow step.
    WorkerCompleted {
        /// Step that produced the worker completion.
        step_id: WorkflowStepId,
        /// Fail-closed worker signal.
        signal: NormalizedSignal,
        /// Concrete artifact updates observed when the worker pass completed.
        artifact_updates: Vec<ArtifactUpdate>,
    },
    /// Record evaluator completion for the current workflow step.
    EvaluatorCompleted {
        /// Step that produced the evaluator completion.
        step_id: WorkflowStepId,
        /// Fail-closed evaluator signal.
        signal: NormalizedSignal,
        /// Concrete artifact updates observed when the evaluator pass completed.
        artifact_updates: Vec<ArtifactUpdate>,
        /// Full evaluator response text, captured when the evaluator emitted Hold.
        /// `None` when the evaluator passed or used a test-double runtime.
        evaluator_output: Option<OutputText>,
    },
    /// Apply the typed failure decision chosen for the current step.
    ApplyFailureDecision {
        /// Step whose failure path is being resolved.
        step_id: WorkflowStepId,
        /// Decision selected by the policy boundary.
        decision: FailureDecision,
    },
    /// Treat a dispatch or completion infrastructure failure as a failing step pass.
    AgentExecutionFailed {
        /// Step whose worker or evaluator pass failed infrastructurally.
        step_id: WorkflowStepId,
        /// Dispatch path whose infrastructure failed.
        kind: DispatchRequestKind,
    },
    /// Shut down the runtime actor loop.
    Shutdown,
}
