//! `OpenRouterTaskRunner`: concrete [`BackgroundTaskRunnerPort`] that spawns
//! OpenRouter task actors for non-Copilot endpoints.

use augur_core::actors::active_model::ActiveModelHandle;
use augur_domain::domain::string_newtypes::{IntentName, PromptText};
use augur_domain::domain::task_types::{AgentSpecName, TaskDepth, TaskRunId};
use augur_domain::domain::traits::BackgroundTaskRunnerPort;
use augur_provider_openrouter::actors::openrouter_orchestrator::handle::{
    OpenRouterEnqueueArgs, OpenRouterOrchestratorHandle,
};
use std::sync::Arc;

/// Supporting services for OpenRouter task dispatch.
pub struct TaskRunnerServices {
    /// OpenRouter orchestrator handle for tracked async dispatch.
    pub orchestrator: OpenRouterOrchestratorHandle,
}

/// Configuration for the `OpenRouterTaskRunner`.
///
/// Holds all wiring-layer handles needed to launch a task. Created once
/// per application session and stored in `EndpointRoutingChatProvider`.
#[derive(bon::Builder)]
pub struct OpenRouterTaskRunnerConfig {
    /// Supporting services injected into each spawned task.
    pub services: TaskRunnerServices,
    /// Active-model handle; provides the current model to each spawned task.
    pub active_model: ActiveModelHandle,
}

/// Concrete [`BackgroundTaskRunnerPort`] that spawns OpenRouter task actors.
///
/// Holds all wiring-layer handles needed to launch a task. Created once per
/// application session and stored in `EndpointRoutingChatProvider`.
pub struct OpenRouterTaskRunner {
    config: OpenRouterTaskRunnerConfig,
}

impl OpenRouterTaskRunner {
    /// Construct a runner from a fully-wired configuration bundle.
    pub fn new(config: OpenRouterTaskRunnerConfig) -> Self {
        OpenRouterTaskRunner { config }
    }

    /// Build enqueue args for an orchestrator-tracked root run.
    fn build_enqueue_args(
        &self,
        agent: AgentSpecName,
        prompt: PromptText,
    ) -> OpenRouterEnqueueArgs {
        OpenRouterEnqueueArgs::builder()
            .agent_name(agent)
            .prompt(prompt)
            .depth(TaskDepth::root())
            .run_id(TaskRunId::new(uuid::Uuid::new_v4().to_string()))
            .maybe_model_override(self.config.active_model.current_model())
            .build()
    }
}

impl BackgroundTaskRunnerPort for OpenRouterTaskRunner {
    /// Fire-and-forget spawn of a background task actor.
    ///
    /// Inputs: `agent` - agent spec name to load; `prompt` - initial user message.
    /// Side effects: spawns a Tokio task; the `JoinHandle` is dropped so the task
    /// runs to completion independently. Output flows via the feed channel.
    fn run(&self, agent: AgentSpecName, prompt: PromptText) {
        let enqueue_args = self.build_enqueue_args(agent, prompt);
        let ack_rx = match self
            .config
            .services
            .orchestrator
            .enqueue_spawn(enqueue_args)
        {
            Ok(receiver) => receiver,
            Err(error) => {
                tracing::warn!("failed to enqueue openrouter run: {error}");
                return;
            }
        };
        tokio::spawn(async move {
            let _ = ack_rx.await;
        });
    }
}

/// Arguments for building an `OpenRouterTaskRunner` at startup.
///
/// Bundles the wiring-layer handles and config needed so the builder
/// stays within the three-parameter limit.
#[derive(bon::Builder)]
pub struct TaskRunnerBuildArgs {
    /// OpenRouter orchestrator handle owned by core runtime wiring.
    pub orchestrator: OpenRouterOrchestratorHandle,
    /// Active-model handle paired with orchestrator task dispatch.
    pub active_model: ActiveModelHandle,
}

/// Result of building an `OpenRouterTaskRunner`.
///
/// Carries the optional runner arc and the active-model handle so the caller
/// can wire the `ActiveModelChanged` listener without a separate spawn call.
pub struct TaskRunnerOutcome {
    /// The constructed runner, if one was built successfully.
    pub runner: Option<Arc<dyn BackgroundTaskRunnerPort>>,
    /// Handle to the active-model actor spawned during construction.
    pub active_model: ActiveModelHandle,
}

/// Build an `OpenRouterTaskRunner` and return a [`TaskRunnerOutcome`].
///
/// Loads the OpenRouter instruction prefix and constructs the runner. The
/// `active_model` field of the outcome always contains a live handle regardless
/// of whether the runner was constructed.
///
/// Inputs: `args` - wiring handles and config.
/// Outputs: [`TaskRunnerOutcome`] - always populated; `runner` is `Some` when
/// construction succeeds (in practice always `Some` on the real wiring path).
pub async fn build_task_runner(args: TaskRunnerBuildArgs) -> TaskRunnerOutcome {
    let services = TaskRunnerServices {
        orchestrator: args.orchestrator,
    };

    let runner = OpenRouterTaskRunner::new(
        OpenRouterTaskRunnerConfig::builder()
            .services(services)
            .active_model(args.active_model.clone())
            .build(),
    );

    TaskRunnerOutcome {
        runner: Some(Arc::new(runner)),
        active_model: args.active_model,
    }
}

// ============================================================================
// Stage 3.2: OpenRouter Hybrid Intent-Action Routing Signatures (M9)
// ============================================================================

use augur_core::actors::orchestrator::ingestion::{
    OrchestratorContext, OrchestratorError, drive_scheduler_tick, submit_execution_plan,
};
use augur_domain::domain::{
    ExecutionPlan, ExecutionPlanError, ExecutionStepId, ExecutionStepSpec, OrchestratorEvent,
    RawStepId, TimeoutConfig,
};

/// Boundary step descriptor used to build an execution plan from wiring input.
#[derive(Clone, Debug, bon::Builder)]
pub struct TaskRequestStep {
    pub step_id: RawStepId,
    pub intent_name: IntentName,
    pub depends_on: Vec<RawStepId>,
    pub required_artifacts: Vec<String>,
    pub produces: Vec<String>,
}

/// Wiring-layer request envelope for execution-plan construction.
#[derive(Clone, Debug, bon::Builder)]
pub struct TaskRequest {
    pub steps: Vec<TaskRequestStep>,
    pub timeout: Option<TimeoutConfig>,
}

/// Build an unvalidated domain `ExecutionPlan` from a wiring `TaskRequest`.
///
/// Preconditions: `request.steps` contains at least one logical step.
/// Postconditions: successful output is unvalidated and must be submitted via orchestrator ingestion.
/// Failure cases: `ExecutionPlanError::EmptyStepId` (from `ExecutionStepId::new`).
pub fn build_execution_plan_for_request(
    request: TaskRequest,
) -> Result<ExecutionPlan, ExecutionPlanError> {
    let mut steps = Vec::with_capacity(request.steps.len());

    for step in request.steps {
        let step_id = ExecutionStepId::new(step.step_id)?;
        let mut depends_on = Vec::with_capacity(step.depends_on.len());
        for dep in step.depends_on {
            depends_on.push(ExecutionStepId::new(dep)?);
        }

        steps.push(ExecutionStepSpec {
            step_id,
            intent_name: step.intent_name.clone(),
            depends_on,
            required_artifacts: step.required_artifacts,
            produces: step.produces,
        });
    }

    Ok(ExecutionPlan::new(steps, request.timeout))
}

/// Wiring adapter that builds and submits execution plans through orchestrator ingestion.
pub struct TaskRunner {
    orchestrator_ctx: OrchestratorContext,
}

impl TaskRunner {
    /// Construct a task runner bound to a specific orchestrator context.
    pub fn new(orchestrator_ctx: OrchestratorContext) -> Self {
        Self { orchestrator_ctx }
    }

    /// Build and submit one request, then return the orchestrator initial event.
    ///
    /// Preconditions: request can be converted into at least one step.
    /// Postconditions: submission path always flows through `submit_execution_plan`.
    /// Failure cases: `OrchestratorError::{InvalidPlan, PersistenceFailed, PlanNotFound, InvariantViolation}`.
    pub fn run(&self, request: TaskRequest) -> Result<OrchestratorEvent, OrchestratorError> {
        let plan = build_execution_plan_for_request(request)
            .map_err(|cause| OrchestratorError::InvalidPlan { cause })?;
        let run_id = submit_execution_plan(plan, self.orchestrator_ctx.clone())?;
        drive_scheduler_tick(run_id, self.orchestrator_ctx.clone())
    }
}
