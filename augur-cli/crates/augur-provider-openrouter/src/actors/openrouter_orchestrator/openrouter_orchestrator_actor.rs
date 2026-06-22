//! OpenRouter orchestrator actor.

use super::assistant_core;
use super::handle::OpenRouterOrchestratorHandle;
use super::openrouter_orchestrator_actor_ops as actor_ops;
use super::openrouter_orchestrator_ops::RunLifecycleLedger;
use crate::actors::llm::handle::LlmHandle;
use augur_domain::ModelId;
use augur_domain::actors::{active_model::ActiveModelHandle, tool::InlineToolExecutor};
use augur_domain::newtypes::Count;
use augur_domain::task_types::{
    AwaitRunResult, InstructionPrefix, RepoRoot, SpawnAgentRequest, TaskRunId,
    TaskRunStatusSnapshot, TaskSignal,
};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc;

const ORCHESTRATOR_CHANNEL_CAPACITY: usize = 64;

/// Spawn-time dependencies owned by the OpenRouter orchestrator actor.
#[derive(Clone, bon::Builder)]
pub struct OpenRouterOrchestratorArgs {
    /// Runtime actor handles used by spawned task runs.
    pub runtime: OrchestratorRuntimeHandles,
    /// Channel senders for user-visible and interactive events.
    pub io: OrchestratorIoChannels,
    /// Immutable OpenRouter task configuration shared by all runs.
    pub config: OrchestratorTaskConfig,
}

/// Runtime actor handles required to spawn an OpenRouter task actor.
#[derive(Clone, bon::Builder)]
pub struct OrchestratorRuntimeHandles {
    /// LLM actor handle used by each spawned OpenRouter task.
    pub llm: LlmHandle,
    /// Active-model handle for run-time model override reads.
    pub active_model: ActiveModelHandle,
    /// Pre-built tool executor provided by wiring/composition.
    pub tool_executor: InlineToolExecutor,
}

/// IO channel dependencies required for OpenRouter task execution.
#[derive(Clone)]
pub struct OrchestratorIoChannels {
    /// Feed output channel for task lifecycle and tool events.
    pub feed_tx: mpsc::Sender<augur_domain::types::FeedEntry>,
}

/// Shared immutable task configuration for orchestrated runs.
#[derive(Clone, bon::Builder)]
pub struct OrchestratorTaskConfig {
    /// Directory allow-list for file-write and list-directory tools.
    pub allowed_dirs: Vec<std::path::PathBuf>,
    /// Instruction prefix prepended to each task request.
    pub instruction_prefix: Arc<InstructionPrefix>,
    /// Repo-root path for resolving agent spec files.
    pub repo_root: RepoRoot,
    /// Maximum number of OpenRouter task workers running in parallel.
    pub max_parallel_workers: usize,
}

/// Commands accepted by the OpenRouter orchestrator actor.
pub enum OpenRouterOrchestratorCommand {
    /// Enqueue and spawn a correlated OpenRouter task run.
    EnqueueSpawn {
        /// Spawn request payload with correlation and dispatch ack channel.
        request: SpawnAgentRequest,
        /// Optional per-run model override.
        model_override: Option<ModelId>,
    },
    /// Correlation notification that a run transitioned to active.
    TransitionToActive {
        /// Correlated run id entering active execution.
        run_id: TaskRunId,
    },
    /// Correlation notification that a run reached terminal state.
    TerminalResult {
        /// Correlated run id for terminal outcome.
        run_id: TaskRunId,
        /// Terminal outcome signal.
        signal: TaskSignal,
    },
    /// Consume terminal state for one correlated run id.
    AwaitRun {
        /// Correlated run id to await.
        run_id: TaskRunId,
        /// One-shot reply sender for await result.
        reply_tx: tokio::sync::oneshot::Sender<AwaitRunResult>,
    },
    /// Consume terminal state for any run id in the provided list.
    AwaitAny {
        /// Candidate correlated run ids.
        run_ids: Vec<TaskRunId>,
        /// One-shot reply sender for await result.
        reply_tx: tokio::sync::oneshot::Sender<AwaitRunResult>,
    },
    /// Query current orchestrator status snapshot.
    QueryStatus {
        /// One-shot reply sender for the status snapshot.
        reply_tx: tokio::sync::oneshot::Sender<TaskRunStatusSnapshot>,
    },
    /// Rotate OpenRouter session context for subsequent requests.
    ResetSession,
    /// Stop the orchestrator command loop and release runtime resources.
    Shutdown,
}

#[derive(bon::Builder)]
pub(super) struct QueuedSpawnRequest {
    pub(super) agent_name: augur_domain::task_types::AgentSpecName,
    pub(super) prompt: augur_domain::PromptText,
    pub(super) depth: augur_domain::task_types::TaskDepth,
    pub(super) run_id: TaskRunId,
    pub(super) terminal_tx: tokio::sync::oneshot::Sender<TaskSignal>,
}

#[derive(bon::Builder)]
pub(super) struct QueuedSpawn {
    pub(super) request: QueuedSpawnRequest,
    pub(super) model_override: Option<ModelId>,
}

#[derive(bon::Builder)]
pub(super) struct AwaitWaiter {
    pub(super) run_ids: Vec<TaskRunId>,
    pub(super) reply_tx: tokio::sync::oneshot::Sender<AwaitRunResult>,
}

#[derive(bon::Builder)]
pub(super) struct RunSchedulingState {
    pub(super) ledger: RunLifecycleLedger,
    pub(super) active_joins: HashMap<TaskRunId, tokio::task::JoinHandle<()>>,
    pub(super) queue: VecDeque<QueuedSpawn>,
    pub(super) await_waiters: VecDeque<AwaitWaiter>,
    pub(super) max_parallel_workers: usize,
}

#[derive(bon::Builder)]
pub(super) struct OpenRouterOrchestratorState {
    pub(super) args: OpenRouterOrchestratorArgs,
    pub(super) scheduling: RunSchedulingState,
    pub(super) session_generation: u64,
    pub(super) self_handle: OpenRouterOrchestratorHandle,
}

#[derive(bon::Builder)]
pub(super) struct BuildOpenRouterTaskArgsInput {
    pub(super) args: OpenRouterOrchestratorArgs,
    pub(super) orchestrator: OpenRouterOrchestratorHandle,
    pub(super) queued_spawn: QueuedSpawn,
    pub(super) session_generation: u64,
}

/// Spawn the OpenRouter orchestrator actor.
///
/// Returns the actor join handle and cloneable command handle.
pub fn spawn(
    args: OpenRouterOrchestratorArgs,
) -> (tokio::task::JoinHandle<()>, OpenRouterOrchestratorHandle) {
    let (cmd_tx, cmd_rx) =
        mpsc::channel::<OpenRouterOrchestratorCommand>(ORCHESTRATOR_CHANNEL_CAPACITY);
    let handle = OpenRouterOrchestratorHandle::new(cmd_tx);
    let max_parallel_workers =
        actor_ops::resolve_max_parallel_workers(Count::of(args.config.max_parallel_workers));
    let state = OpenRouterOrchestratorState::builder()
        .args(args)
        .scheduling(
            RunSchedulingState::builder()
                .ledger(RunLifecycleLedger::default())
                .active_joins(HashMap::new())
                .queue(VecDeque::new())
                .await_waiters(VecDeque::new())
                .max_parallel_workers(*max_parallel_workers)
                .build(),
        )
        .session_generation(0)
        .self_handle(handle.clone())
        .build();
    let join = tokio::spawn(run_loop(cmd_rx, state));
    (join, handle)
}

/// Main orchestrator run loop.
///
/// Owns pending/active/terminal lifecycle state and OpenRouter session generation.
async fn run_loop(
    cmd_rx: mpsc::Receiver<OpenRouterOrchestratorCommand>,
    state: OpenRouterOrchestratorState,
) {
    assistant_core::run_loop(cmd_rx, state).await
}
