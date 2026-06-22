//! Domain foundation types for agent task spawning and agent specification.
//!
//! Provides the depth-bounded recursion type [`TaskDepth`], task lifecycle signals
//! [`TaskSignal`], agent specification types ([`AgentSpec`], [`AgentSpecMeta`],
//! [`AgentSpecName`], [`AgentToolSet`], [`AgentInstructions`]),
//! instruction context wrappers ([`InstructionPrefix`]), the spawn request
//! envelope [`SpawnAgentRequest`], and the [`SpawnAgentHandle`] channel wrapper
//! used to dispatch sub-agents.

use crate::domain::string_newtypes::{ArtifactData, ArtifactName, IntentName};
use crate::domain::{AccumulatedText, Message, ModelId, OutputText, PromptText};
use std::any::Any;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

/// Maximum allowed nesting depth for spawned sub-agents.
///
/// A `TaskDepth` value may not exceed this constant; `increment` returns `None`
/// when the current depth is already at or above this value.
pub const MAX_TASK_DEPTH: u8 = 8;

/// Correlation identifier for a spawned task run.
///
/// Wraps `String` so orchestrator run identifiers cannot be confused with
/// other string domain values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskRunId(String);

impl TaskRunId {
    /// Wrap any value that converts to `String` into a `TaskRunId`.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for TaskRunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for TaskRunId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Depth counter for sub-agent task nesting.
///
/// Wraps a `u8` in a semantic newtype so that recursion depth cannot be confused
/// with other numeric domain values. Bounded at [`MAX_TASK_DEPTH`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskDepth(pub u8);

impl TaskDepth {
    /// Construct the root depth (zero).
    ///
    /// Use this as the starting depth when spawning a top-level agent task.
    pub fn root() -> Self {
        Self(0)
    }

    /// Attempt to produce the next depth level.
    ///
    /// Returns `Some(TaskDepth(self.0 + 1))` when `self.0 < MAX_TASK_DEPTH`,
    /// or `None` when the maximum has been reached, preventing further nesting.
    pub fn increment(&self) -> Option<Self> {
        if self.0 >= MAX_TASK_DEPTH {
            None
        } else {
            Some(Self(self.0 + 1))
        }
    }
}

/// Lifecycle outcome signal for a completed, failed, or cancelled agent task.
///
/// Sent over a `tokio::sync::oneshot` channel from the spawned agent back to
/// the caller once the task terminates.
#[derive(Clone, Debug)]
pub enum TaskSignal {
    /// The agent completed its work and produced accumulated output.
    Completed {
        /// Full accumulated text produced by the agent turn.
        output: AccumulatedText,
    },
    /// The agent encountered an error and could not complete its work.
    Failed {
        /// Human-readable reason explaining the failure.
        reason: OutputText,
    },
    /// The agent task was cancelled before it could finish.
    Cancelled,
}

/// Dispatch state for an enqueued run relative to the worker-cap scheduler.
#[derive(Clone, Debug)]
pub enum TaskDispatchState {
    /// Run was queued because all worker slots were occupied.
    Queued {
        /// Zero-based position in the queue at acknowledgement time.
        position: usize,
    },
    /// Run was accepted and dispatched immediately.
    Dispatched,
}

/// Queue-capacity snapshot returned at spawn acknowledgement time.
#[derive(Clone, Debug, bon::Builder)]
pub struct TaskQueueSnapshot {
    /// Maximum number of task workers that may run in parallel.
    pub max_parallel_workers: usize,
    /// Number of currently active task workers.
    pub active_runs: usize,
    /// Number of queued runs awaiting a free worker slot.
    pub queued_runs: usize,
}

/// Spawn acknowledgement payload with deterministic run correlation metadata.
#[derive(Clone, Debug, bon::Builder)]
pub struct SpawnDispatchStatus {
    /// Correlated run identifier for the accepted request.
    pub run_id: TaskRunId,
    /// Dispatch-vs-queued state at acknowledgement time.
    pub dispatch_state: TaskDispatchState,
    /// Queue and cap metadata snapshot for backpressure visibility.
    pub queue_snapshot: TaskQueueSnapshot,
}

/// Dispatch acknowledgement payload returned for spawn requests.
///
/// This is intentionally distinct from [`TaskSignal`] so request-dispatch
/// acknowledgement can evolve independently from terminal task lifecycle output.
#[derive(Debug)]
pub enum SpawnAgentAck {
    /// Spawn request was acknowledged and carries dispatch metadata.
    Completed {
        /// Run correlation and queue-capacity metadata for this request.
        status: SpawnDispatchStatus,
    },
    /// Spawn request was rejected.
    Failed {
        /// Human-readable reason explaining the rejection.
        reason: OutputText,
    },
    /// Spawn request was cancelled before handling.
    Cancelled,
}

/// Request-scoped channels for one spawn lifecycle.
///
/// `ack_tx` carries dispatch acknowledgement while `terminal_tx` carries the
/// terminal task signal for this specific request/run correlation.
#[derive(bon::Builder)]
pub struct SpawnAgentChannels {
    /// Channel on which the dispatch layer reports spawn acknowledgement.
    pub ack_tx: tokio::sync::oneshot::Sender<SpawnAgentAck>,
    /// Channel on which the task runtime reports terminal completion/failure.
    pub terminal_tx: tokio::sync::oneshot::Sender<TaskSignal>,
}

/// Terminal await result for one correlated run id.
#[derive(Clone, Debug)]
pub enum AwaitRunResult {
    /// A terminal payload was consumed for the requested run id.
    ConsumedTerminal {
        /// Correlated run id whose terminal payload was consumed.
        run_id: TaskRunId,
        /// Terminal lifecycle signal consumed from the ledger.
        signal: TaskSignal,
    },
    /// The run already had its terminal payload consumed by a prior await call.
    AlreadyConsumed {
        /// Correlated run id already consumed.
        run_id: TaskRunId,
    },
    /// No known run exists for the requested run id.
    UnknownRun {
        /// Correlated run id that is unknown to the orchestrator.
        run_id: TaskRunId,
    },
}

impl AwaitRunResult {
    /// Borrow the correlated run id associated with this await result.
    pub fn run_id(&self) -> &TaskRunId {
        match self {
            Self::ConsumedTerminal { run_id, .. }
            | Self::AlreadyConsumed { run_id }
            | Self::UnknownRun { run_id } => run_id,
        }
    }
}

/// Lifecycle state snapshot for one tracked run id.
#[derive(Clone, Debug)]
pub enum TaskRunLifecycleState {
    /// Run has been accepted but has not started execution.
    Pending,
    /// Run is actively executing.
    Active,
    /// Run completed, failed, or cancelled and terminal payload is retained.
    TerminalReady {
        /// Terminal signal retained for await consumption.
        signal: TaskSignal,
    },
    /// Run terminal payload has already been consumed through await.
    TerminalConsumed,
}

/// Status entry for one tracked run.
#[derive(Clone, Debug, bon::Builder)]
pub struct TaskRunStatusEntry {
    /// Correlated run id.
    pub run_id: TaskRunId,
    /// Current lifecycle state.
    pub state: TaskRunLifecycleState,
}

/// Orchestrator status snapshot used by status/list APIs.
#[derive(Clone, Debug, bon::Builder)]
pub struct TaskRunStatusSnapshot {
    /// Maximum number of worker slots configured for parallel task execution.
    pub max_parallel_workers: usize,
    /// Number of currently active worker slots.
    pub active_runs: usize,
    /// Number of queued requests waiting for dispatch.
    pub queued_runs: usize,
    /// Total number of retained terminal results waiting for consumption.
    pub terminal_ready_runs: usize,
    /// Per-run lifecycle status entries.
    pub runs: Vec<TaskRunStatusEntry>,
}

/// Port for orchestrator-backed task lifecycle operations used by tools.
///
/// This trait decouples built-in tools from actor module internals while
/// preserving deterministic run-id based spawn/await/status semantics.
pub trait TaskOrchestratorPort: Send + Sync {
    /// Enqueue a request to consume terminal output for one run id.
    fn await_run(&self, run_id: TaskRunId) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>>;
    /// Enqueue a request to consume terminal output for any candidate run id.
    fn await_any(
        &self,
        run_ids: Vec<TaskRunId>,
    ) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>>;
    /// Enqueue a request for a scheduler and lifecycle status snapshot.
    fn query_status(&self) -> anyhow::Result<oneshot::Receiver<TaskRunStatusSnapshot>>;
}

/// Semantic identifier for a named agent specification.
///
/// Used as a key to look up an [`AgentSpec`] by name in a registry.
/// Wraps `String` so that spec names cannot be accidentally confused
/// with other string domain values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentSpecName(String);

impl AgentSpecName {
    /// Wrap any value that converts to `String` into an `AgentSpecName`.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for AgentSpecName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for AgentSpecName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// The set of tools made available to a spawned agent.
///
/// `All` grants access to every registered tool; `Named` restricts execution
/// to the explicitly listed tool spec names.
#[derive(Clone, Debug)]
pub enum AgentToolSet {
    /// Grant the agent access to all registered tools.
    All,
    /// Restrict the agent to only the listed tool spec names.
    Named(Vec<AgentSpecName>),
}

/// Metadata accompanying an agent specification.
///
/// Describes the agent's purpose, preferred model, and permitted tool set.
#[derive(bon::Builder, Clone, Debug)]
pub struct AgentSpecMeta {
    /// Human-readable description of the agent's role and responsibilities.
    pub description: OutputText,
    /// Optional model identifier override; `None` uses the session default.
    pub model: Option<ModelId>,
    /// The set of tools available to this agent during execution.
    pub tools: AgentToolSet,
}

/// Free-form instruction text injected into an agent's system prompt.
///
/// Wraps `String` so that instruction content cannot be confused with other
/// string domain values such as prompts or tool descriptions.
#[derive(Clone, Debug)]
pub struct AgentInstructions(String);

impl AgentInstructions {
    /// Wrap any value that converts to `String` into `AgentInstructions`.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for AgentInstructions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for AgentInstructions {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Complete specification for a named agent: identity, metadata, and instructions.
///
/// Registered in an agent spec registry and looked up by [`AgentSpecName`] at
/// spawn time to configure a sub-agent task.
#[derive(bon::Builder, Clone, Debug)]
pub struct AgentSpec {
    /// Unique name identifying this agent specification.
    pub name: AgentSpecName,
    /// Descriptive metadata including model and tool set preferences.
    pub meta: AgentSpecMeta,
    /// System-level instruction text injected before the user prompt.
    pub instructions: AgentInstructions,
}

/// Ordered list of [`Message`] values prepended to an agent's conversation context.
///
/// Wraps `Vec<Message>` so that an instruction prefix cannot be accidentally
/// passed where a plain message list is expected.
pub struct InstructionPrefix(pub Vec<Message>);

impl Deref for InstructionPrefix {
    type Target = Vec<Message>;

    fn deref(&self) -> &Vec<Message> {
        &self.0
    }
}

/// Request envelope for spawning a sub-agent task.
///
/// Carries the agent name, user prompt, current nesting depth, and a oneshot
/// channel bundle for dispatch acknowledgement and terminal completion.
#[derive(bon::Builder)]
pub struct SpawnAgentRequest {
    /// Name of the agent spec to look up and spawn.
    pub agent_name: AgentSpecName,
    /// User-supplied prompt text submitted to the spawned agent.
    pub prompt: PromptText,
    /// Nesting depth at which this agent is being spawned.
    pub depth: TaskDepth,
    /// Correlation id for this spawn request run.
    pub run_id: TaskRunId,
    /// Request-scoped lifecycle channels for dispatch and terminal signals.
    pub channels: SpawnAgentChannels,
}

/// Relative path to an instruction file that should be injected as a prefix message.
///
/// Wraps `String` so that file paths cannot be confused with arbitrary string
/// domain values. Paths are relative to [`RepoRoot`].
#[derive(Clone, Debug)]
pub struct InstructionFilePath(pub String);

impl InstructionFilePath {
    /// Wrap any value that converts to `String` into an `InstructionFilePath`.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for InstructionFilePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for InstructionFilePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Absolute path to the repository root directory.
///
/// Wraps `String` so that the repo root cannot be accidentally passed where
/// a plain path or other domain string is expected.
#[derive(Clone, Debug)]
pub struct RepoRoot(pub String);

impl RepoRoot {
    /// Wrap any value that converts to `String` into a `RepoRoot`.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for RepoRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for RepoRoot {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Domain-native opaque handle to a cache provider.
///
/// Wraps an erased `Arc<dyn Any + Send + Sync>` so that the domain layer can
/// carry a cache reference without depending on the actors layer. Callers in
/// the wiring layer are responsible for constructing this from a concrete cache
/// actor handle.
#[derive(Clone)]
pub struct CacheHandle(pub Arc<dyn Any + Send + Sync>);

/// A message compactor function that takes a full message list (including any
/// prepended system prompts / instruction prefixes) and an optional model ID,
/// then returns a compacted list that fits within the provider's context window.
///
/// Implementations should preserve the leading system prompt and drop the
/// oldest conversation turns first. The OpenRouter provider supplies its own
/// compactor via [`MessageCompactor`].  The model ID is forwarded to per-model
/// config resolution (compaction target, strip fraction) so the correct budget
/// for the active model is used.
pub type MessageCompactor =
    Arc<dyn Fn(Vec<Message>, Option<ModelId>) -> Vec<Message> + Send + Sync>;

/// Optional runtime extensions injected into a spawned agent context.
///
/// Carries a shared cache handle, an optional instruction prefix that should
/// be prepended to the agent's conversation history before the user prompt,
/// and an optional message compactor for manual `/compact` support.
#[derive(Clone)]
pub struct AgentExtensions {
    /// Optional cache handle granting the agent access to file snapshot state.
    pub cache: Option<CacheHandle>,
    /// Optional shared instruction prefix prepended to the conversation context.
    pub instruction_prefix: Option<Arc<InstructionPrefix>>,
    /// Optional message compactor for manual `/compact` command support.
    ///
    /// When set, the agent actor calls this function with the current
    /// conversation messages and returns the compacted result. Used by
    /// the OpenRouter provider to compact messages using its own compaction
    /// logic. The Copilot SDK path uses its own SDK compaction and does not
    /// use this field.
    pub message_compactor: Option<MessageCompactor>,
}

/// Channel handle for requesting sub-agent spawns from the task actor main loop.
///
/// Wraps a `tokio::sync::mpsc::Sender<SpawnAgentRequest>` so that callers cannot
/// accidentally pass a raw sender where a typed handle is expected. The actor
/// that owns the receiver is responsible for actually spawning and driving the
/// sub-agent; the tool only sends the request and then awaits the oneshot reply.
#[derive(Clone, Debug)]
pub struct SpawnAgentHandle(pub mpsc::Sender<SpawnAgentRequest>);

impl SpawnAgentHandle {
    /// Send a sub-agent spawn request to the actor main loop.
    ///
    /// Awaits channel capacity. Returns an error if the receiving end has been dropped.
    /// The caller should then await request-scoped lifecycle oneshots for
    /// dispatch acknowledgement and terminal state.
    pub async fn send(
        &self,
        req: SpawnAgentRequest,
    ) -> Result<(), mpsc::error::SendError<SpawnAgentRequest>> {
        self.0.send(req).await
    }
}

// ============================================================================
// Phase 1: Execution Plan Domain Contracts
// ============================================================================

use std::collections::BTreeMap;

/// Raw transport/input wrapper for step identifiers prior to validation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RawStepId {
    /// Unvalidated step-id payload captured from input/transport.
    pub inner: String,
}

impl RawStepId {
    /// Wrap raw step-id input before validation.
    pub fn new(inner: impl Into<String>) -> Self {
        Self {
            inner: inner.into(),
        }
    }
}

/// Semantic identifier for an execution step within a plan.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct ExecutionStepId(
    /// Validated step identifier payload.
    String,
);

impl ExecutionStepId {
    /// Constructs a validated step id.
    pub fn new(value: RawStepId) -> Result<Self, ExecutionPlanError> {
        if value.inner.is_empty() {
            return Err(ExecutionPlanError::EmptyStepId);
        }
        Ok(Self(value.inner))
    }
}

impl AsRef<str> for ExecutionStepId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExecutionStepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
/// Semantic identifier for a single execution run.
#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "String", into = "String")]
pub struct RunId(
    /// Validated run identifier payload.
    String,
);

impl RunId {
    /// Construct a validated run id.
    pub fn new(value: impl Into<String>) -> Result<Self, ExecutionPlanError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ExecutionPlanError::EmptyRunId);
        }
        Ok(Self(value))
    }
}

impl AsRef<str> for RunId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RunId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err("run id must not be empty".to_string());
        }
        Ok(Self(value))
    }
}

impl From<RunId> for String {
    fn from(value: RunId) -> Self {
        value.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Millisecond duration wrapper.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct DurationMs(
    /// Timeout duration in milliseconds.
    pub u64,
);

impl From<u64> for DurationMs {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Borrowed semantic view of a step artifact name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactNameRef<'a>(&'a str);

impl AsRef<str> for ArtifactNameRef<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl fmt::Display for ArtifactNameRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Borrowed semantic view of a step artifact payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactDataRef<'a>(&'a str);

impl AsRef<str> for ArtifactDataRef<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl fmt::Display for ArtifactDataRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Composite identity for one execution step within one run.
///
/// Bundles run_id and step_id so call sites can stay within the three-
/// parameter rule while preserving domain semantics at actor/persistence
/// boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StepKey {
    /// Execution run identifier.
    pub run_id: RunId,
    /// Step identifier within the run.
    pub step_id: ExecutionStepId,
}

impl StepKey {
    /// Construct a composite step key from run and step identifiers.
    pub fn new(run_id: RunId, step_id: ExecutionStepId) -> Self {
        Self { run_id, step_id }
    }
}

/// Deterministic map alias used by plan/runtime state.
pub type Map<K, V> = BTreeMap<K, V>;

/// Artifact payload produced by completed steps.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawStepArtifact")]
pub struct StepArtifact {
    /// Artifact identifier string.
    name: ArtifactName,
    /// Artifact payload content.
    data: ArtifactData,
}

impl StepArtifact {
    /// Build a validated step artifact.
    pub fn new(
        name: impl Into<ArtifactName>,
        data: impl Into<ArtifactData>,
    ) -> Result<Self, ExecutionPlanError> {
        let name: ArtifactName = name.into();
        if name.is_empty() {
            return Err(ExecutionPlanError::EmptyArtifactName);
        }

        Ok(Self {
            name,
            data: data.into(),
        })
    }

    /// Borrow the artifact name as a semantic reference wrapper.
    pub fn name(&self) -> ArtifactNameRef<'_> {
        ArtifactNameRef(&self.name)
    }

    /// Borrow the artifact payload as a semantic reference wrapper.
    pub fn data(&self) -> ArtifactDataRef<'_> {
        ArtifactDataRef(&self.data)
    }
}

#[derive(serde::Deserialize)]
struct RawStepArtifact {
    name: ArtifactName,
    data: ArtifactData,
}

impl TryFrom<RawStepArtifact> for StepArtifact {
    type Error = String;

    fn try_from(value: RawStepArtifact) -> Result<Self, Self::Error> {
        if value.name.is_empty() {
            return Err("step artifact name must not be empty".to_string());
        }
        Ok(Self {
            name: value.name,
            data: value.data,
        })
    }
}

/// Optional timeout constraints for plan and step execution.
#[derive(Clone, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct TimeoutConfig {
    /// Optional global timeout for the full execution plan.
    pub total_timeout_ms: Option<DurationMs>,
    /// Optional timeout applied to each individual step.
    pub per_step_timeout_ms: Option<DurationMs>,
}

/// Lifecycle state of a step in a plan run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StepStatus {
    /// Step has not started yet.
    Pending,
    /// Step is currently executing.
    Running,
    /// Step finished successfully.
    Completed,
    /// Step terminated with an error.
    Failed,
}

/// Immutable static specification of one execution step.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionStepSpec {
    /// Unique step identifier.
    pub step_id: ExecutionStepId,
    /// Planner-selected intent label for the step.
    pub intent_name: IntentName,
    /// Step dependencies that must complete first.
    pub depends_on: Vec<ExecutionStepId>,
    /// Artifact names required as inputs.
    pub required_artifacts: Vec<String>,
    /// Artifact names produced on successful completion.
    pub produces: Vec<String>,
}

/// Raw unvalidated execution plan aggregate.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionPlan {
    /// Ordered step specifications for the execution plan.
    pub steps: Vec<ExecutionStepSpec>,
    /// Plan-level and per-step timeout configuration.
    pub timeout: TimeoutConfig,
}

impl ExecutionPlan {
    /// Construct an unvalidated execution plan aggregate.
    pub fn new(steps: Vec<ExecutionStepSpec>, timeout: Option<TimeoutConfig>) -> Self {
        Self {
            steps,
            timeout: timeout.unwrap_or_default(),
        }
    }
}

/// Typestate wrapper proving all plan invariants passed validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedPlan {
    inner: ExecutionPlan,
}

impl ValidatedPlan {
    /// Borrow the validated execution plan.
    pub fn inner(&self) -> &ExecutionPlan {
        &self.inner
    }

    /// Consume the wrapper and return the validated plan.
    pub fn into_inner(self) -> ExecutionPlan {
        self.inner
    }

    /// Build a typestate wrapper from a plan that has already passed validation.
    pub(crate) fn from_validated(inner: ExecutionPlan) -> Self {
        Self { inner }
    }
}

/// Validation and domain failure cases for execution plans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionPlanError {
    /// A plan contains two or more steps with the same step id.
    DuplicateStepId {
        /// The duplicated step identifier.
        step_id: ExecutionStepId,
    },
    /// A step depends on another step id that does not exist in the plan.
    UndefinedStepReference {
        /// Step declaring the invalid dependency.
        step_id: ExecutionStepId,
        /// Missing dependency identifier.
        referenced: ExecutionStepId,
    },
    /// A step requires an artifact name never produced by any predecessor.
    UndeclaredArtifact {
        /// Step whose requirement cannot be satisfied.
        step_id: ExecutionStepId,
        /// Required artifact that was not declared by any producer.
        artifact: String,
    },
    /// Dependency edges form a cycle and cannot be topologically ordered.
    CyclicDependency {
        /// Renderable cycle path in dependency order.
        cycle_path: Vec<ExecutionStepId>,
    },
    /// A timeout configuration value is invalid.
    InvalidTimeout {
        /// Timeout field name (`total_timeout_ms` or `per_step_timeout_ms`).
        field: String,
        /// Invalid timeout value.
        value: DurationMs,
    },
    /// Step id string is empty.
    EmptyStepId,
    /// Run id string is empty.
    EmptyRunId,
    /// Artifact name string is empty.
    EmptyArtifactName,
    /// A run with the same deterministic id is already registered.
    PlanAlreadyExists {
        /// Existing run identifier that caused the collision.
        run_id: RunId,
    },
}

impl std::fmt::Display for ExecutionPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_execution_plan_error(f, self)
    }
}

fn write_execution_plan_error(
    f: &mut std::fmt::Formatter<'_>,
    error: &ExecutionPlanError,
) -> std::fmt::Result {
    match error {
        ExecutionPlanError::DuplicateStepId { .. }
        | ExecutionPlanError::UndefinedStepReference { .. } => {
            write_dependency_reference_error(f, error)
        }
        ExecutionPlanError::UndeclaredArtifact { .. }
        | ExecutionPlanError::CyclicDependency { .. } => write_dependency_content_error(f, error),
        ExecutionPlanError::InvalidTimeout { .. }
        | ExecutionPlanError::PlanAlreadyExists { .. } => write_runtime_plan_error(f, error),
        ExecutionPlanError::EmptyStepId
        | ExecutionPlanError::EmptyRunId
        | ExecutionPlanError::EmptyArtifactName => write_empty_value_error(f, error),
    }
}

fn write_dependency_reference_error(
    f: &mut std::fmt::Formatter<'_>,
    error: &ExecutionPlanError,
) -> std::fmt::Result {
    match error {
        ExecutionPlanError::DuplicateStepId { step_id } => write_duplicate_step_id(f, step_id),
        ExecutionPlanError::UndefinedStepReference {
            step_id,
            referenced,
        } => write_undefined_step_reference(f, step_id, referenced),
        _ => write!(f, "execution plan dependency reference error"),
    }
}

fn write_dependency_content_error(
    f: &mut std::fmt::Formatter<'_>,
    error: &ExecutionPlanError,
) -> std::fmt::Result {
    match error {
        ExecutionPlanError::UndeclaredArtifact { step_id, artifact } => {
            write_undeclared_artifact(f, step_id, artifact)
        }
        ExecutionPlanError::CyclicDependency { cycle_path } => {
            write_cyclic_dependency(f, cycle_path)
        }
        _ => write!(f, "execution plan dependency content error"),
    }
}

fn write_runtime_plan_error(
    f: &mut std::fmt::Formatter<'_>,
    error: &ExecutionPlanError,
) -> std::fmt::Result {
    match error {
        ExecutionPlanError::InvalidTimeout { field, value } => {
            write_invalid_timeout(f, field, value)
        }
        ExecutionPlanError::PlanAlreadyExists { run_id } => write_plan_already_exists(f, run_id),
        _ => write!(f, "execution plan runtime error"),
    }
}

fn write_empty_value_error(
    f: &mut std::fmt::Formatter<'_>,
    error: &ExecutionPlanError,
) -> std::fmt::Result {
    match error {
        ExecutionPlanError::EmptyStepId => write_empty_step_id(f),
        ExecutionPlanError::EmptyRunId => write_empty_run_id(f),
        ExecutionPlanError::EmptyArtifactName => write_empty_artifact_name(f),
        _ => write!(f, "execution plan value cannot be empty"),
    }
}

fn write_duplicate_step_id(
    f: &mut std::fmt::Formatter<'_>,
    step_id: &ExecutionStepId,
) -> std::fmt::Result {
    write!(
        f,
        "duplicate step id in execution plan: {}",
        step_id.as_ref()
    )
}

fn write_undefined_step_reference(
    f: &mut std::fmt::Formatter<'_>,
    step_id: &ExecutionStepId,
    referenced: &ExecutionStepId,
) -> std::fmt::Result {
    write!(
        f,
        "step {} references undefined dependency {}",
        step_id.as_ref(),
        referenced.as_ref()
    )
}

fn write_undeclared_artifact(
    f: &mut std::fmt::Formatter<'_>,
    step_id: &ExecutionStepId,
    artifact: &str,
) -> std::fmt::Result {
    write!(
        f,
        "step {} requires undeclared artifact {}",
        step_id.as_ref(),
        artifact
    )
}

fn write_cyclic_dependency(
    f: &mut std::fmt::Formatter<'_>,
    cycle_path: &[ExecutionStepId],
) -> std::fmt::Result {
    let rendered = cycle_path
        .iter()
        .map(|id| id.as_ref().to_owned())
        .collect::<Vec<_>>()
        .join(" -> ");
    write!(f, "cyclic dependency detected: {rendered}")
}

fn write_invalid_timeout(
    f: &mut std::fmt::Formatter<'_>,
    field: &str,
    value: &DurationMs,
) -> std::fmt::Result {
    write!(f, "invalid timeout for {field}: {}", value.0)
}

fn write_empty_step_id(f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "step id cannot be empty")
}

fn write_empty_run_id(f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "run id cannot be empty")
}

fn write_empty_artifact_name(f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "artifact name cannot be empty")
}

fn write_plan_already_exists(f: &mut std::fmt::Formatter<'_>, run_id: &RunId) -> std::fmt::Result {
    write!(
        f,
        "execution plan already exists for run {}",
        run_id.as_ref()
    )
}
