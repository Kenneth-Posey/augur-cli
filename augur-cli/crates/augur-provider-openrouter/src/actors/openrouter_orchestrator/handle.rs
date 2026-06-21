//! Handle for the OpenRouter orchestrator actor.

use super::openrouter_orchestrator_actor::OpenRouterOrchestratorCommand;
use augur_domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::task_types::{
    AgentSpecName, AwaitRunResult, SpawnAgentAck, SpawnAgentChannels, SpawnAgentRequest, TaskDepth,
    TaskOrchestratorPort, TaskRunId, TaskRunStatusSnapshot, TaskSignal,
};
use augur_domain::{ModelId, PromptText};
use tokio::sync::{mpsc, oneshot};

/// Arguments for an orchestrator-enqueued spawn request.
///
/// Carries the complete dispatch envelope needed to build a `SpawnAgentRequest`.
#[derive(bon::Builder)]
pub struct OpenRouterEnqueueArgs {
    /// Name of the agent specification to run.
    pub agent_name: AgentSpecName,
    /// Prompt sent as the first user message to the spawned agent.
    pub prompt: PromptText,
    /// Task depth used for recursive spawn limiting.
    pub depth: TaskDepth,
    /// Correlation id for this task run.
    pub run_id: TaskRunId,
    /// Optional model override for this run.
    pub model_override: Option<ModelId>,
}

/// Cloneable command handle for the OpenRouter orchestrator actor.
#[derive(Clone)]
pub struct OpenRouterOrchestratorHandle {
    cmd_tx: mpsc::Sender<OpenRouterOrchestratorCommand>,
}

impl OpenRouterOrchestratorHandle {
    /// Construct a handle from the actor command sender.
    pub(crate) fn new(cmd_tx: mpsc::Sender<OpenRouterOrchestratorCommand>) -> Self {
        Self { cmd_tx }
    }

    /// Enqueue a new OpenRouter task spawn and return the dispatch-ack receiver.
    ///
    /// The returned receiver resolves when the orchestrator acknowledges dispatch.
    /// Terminal completion is delivered via orchestrator terminal correlation.
    pub fn enqueue_spawn(
        &self,
        args: OpenRouterEnqueueArgs,
    ) -> anyhow::Result<oneshot::Receiver<SpawnAgentAck>> {
        let (ack_tx, ack_rx) = oneshot::channel::<SpawnAgentAck>();
        let (terminal_tx, _terminal_rx) = oneshot::channel::<TaskSignal>();
        let request = SpawnAgentRequest::builder()
            .agent_name(args.agent_name)
            .prompt(args.prompt)
            .depth(args.depth)
            .run_id(args.run_id)
            .channels(
                SpawnAgentChannels::builder()
                    .ack_tx(ack_tx)
                    .terminal_tx(terminal_tx)
                    .build(),
            )
            .build();
        self.cmd_tx
            .try_send(OpenRouterOrchestratorCommand::EnqueueSpawn {
                request,
                model_override: args.model_override,
            })
            .map_err(|e| anyhow::anyhow!("openrouter orchestrator queue unavailable: {e}"))?;
        Ok(ack_rx)
    }

    /// Enqueue an already-built spawn request envelope.
    ///
    /// Used by wiring-owned spawn-agent bridges that forward tool-channel
    /// requests directly into the orchestrator without rebuilding correlation.
    pub fn enqueue_request(
        &self,
        request: SpawnAgentRequest,
        model_override: Option<ModelId>,
    ) -> anyhow::Result<()> {
        match self
            .cmd_tx
            .try_send(OpenRouterOrchestratorCommand::EnqueueSpawn {
                request,
                model_override,
            }) {
            Ok(()) => Ok(()),
            Err(tokio::sync::mpsc::error::TrySendError::Full(
                OpenRouterOrchestratorCommand::EnqueueSpawn { request, .. },
            ))
            | Err(tokio::sync::mpsc::error::TrySendError::Closed(
                OpenRouterOrchestratorCommand::EnqueueSpawn { request, .. },
            )) => {
                let run_id = request.run_id.clone();
                let _ = request.channels.ack_tx.send(SpawnAgentAck::Failed {
                    reason: OutputText::new(format!(
                        "task dispatch failed: run_id={} reason=openrouter orchestrator queue unavailable",
                        run_id.as_ref()
                    )),
                });
                Err(anyhow::anyhow!(
                    "openrouter orchestrator queue unavailable for run_id={}",
                    run_id.as_ref()
                ))
            }
            Err(error) => Err(anyhow::anyhow!(
                "openrouter orchestrator queue unavailable: {error}"
            )),
        }
    }

    /// Await one correlated run id and consume its terminal payload.
    pub fn await_run(
        &self,
        run_id: TaskRunId,
    ) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>> {
        let (reply_tx, reply_rx) = oneshot::channel::<AwaitRunResult>();
        self.cmd_tx
            .try_send(OpenRouterOrchestratorCommand::AwaitRun { run_id, reply_tx })
            .map_err(|e| anyhow::anyhow!("openrouter orchestrator queue unavailable: {e}"))?;
        Ok(reply_rx)
    }

    /// Await any terminal completion from a candidate run-id list.
    pub fn await_any(
        &self,
        run_ids: Vec<TaskRunId>,
    ) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>> {
        let (reply_tx, reply_rx) = oneshot::channel::<AwaitRunResult>();
        self.cmd_tx
            .try_send(OpenRouterOrchestratorCommand::AwaitAny { run_ids, reply_tx })
            .map_err(|e| anyhow::anyhow!("openrouter orchestrator queue unavailable: {e}"))?;
        Ok(reply_rx)
    }

    /// Request a status snapshot of queued/active/terminal run ids.
    pub fn query_status(&self) -> anyhow::Result<oneshot::Receiver<TaskRunStatusSnapshot>> {
        let (reply_tx, reply_rx) = oneshot::channel::<TaskRunStatusSnapshot>();
        self.cmd_tx
            .try_send(OpenRouterOrchestratorCommand::QueryStatus { reply_tx })
            .map_err(|e| anyhow::anyhow!("openrouter orchestrator queue unavailable: {e}"))?;
        Ok(reply_rx)
    }

    /// Rotate OpenRouter orchestrator session context.
    ///
    /// This command is non-blocking and only enqueues a reset request.
    pub fn reset_session(&self) -> anyhow::Result<()> {
        self.cmd_tx
            .try_send(OpenRouterOrchestratorCommand::ResetSession)
            .map_err(|e| anyhow::anyhow!("openrouter orchestrator queue unavailable: {e}"))
    }

    /// Stop the orchestrator run loop.
    pub fn shutdown(&self) -> anyhow::Result<()> {
        self.cmd_tx
            .try_send(OpenRouterOrchestratorCommand::Shutdown)
            .map_err(|e| anyhow::anyhow!("openrouter orchestrator queue unavailable: {e}"))
    }

    /// Notify the orchestrator that a run transitioned to active execution.
    pub fn transition_to_active(&self, run_id: TaskRunId) {
        let _ = self
            .cmd_tx
            .try_send(OpenRouterOrchestratorCommand::TransitionToActive { run_id });
    }

    /// Record terminal lifecycle state for a correlated run id.
    pub fn record_terminal_result(&self, run_id: TaskRunId, signal: TaskSignal) {
        let _ = self
            .cmd_tx
            .try_send(OpenRouterOrchestratorCommand::TerminalResult { run_id, signal });
    }
}

impl TaskOrchestratorPort for OpenRouterOrchestratorHandle {
    fn await_run(&self, run_id: TaskRunId) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>> {
        OpenRouterOrchestratorHandle::await_run(self, run_id)
    }

    fn await_any(
        &self,
        run_ids: Vec<TaskRunId>,
    ) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>> {
        OpenRouterOrchestratorHandle::await_any(self, run_ids)
    }

    fn query_status(&self) -> anyhow::Result<oneshot::Receiver<TaskRunStatusSnapshot>> {
        OpenRouterOrchestratorHandle::query_status(self)
    }
}
