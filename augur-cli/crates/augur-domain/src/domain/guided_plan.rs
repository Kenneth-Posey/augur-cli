//! Domain types for guided plan execution.
//!
//! Defines the configuration parsed from YAML frontmatter in plan files and the
//! runtime event and status types used by `GuidedPlanActor` and the TUI.

use crate::domain::{
    AgentName, FailureReason, HookIndex, IsPredicate, OutputText, PhaseIndex, PhaseName, PlanName,
    PlanPhaseId, PromptText, ReworkReason, ShellCommand,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;

// ── Hook configuration ────────────────────────────────────────────────────────

/// Controls what happens when a hook reports a non-passing outcome (non-zero
/// exit code for subprocess hooks, or a session-level failure for agent hooks).
///
/// Applies to infrastructure failures only. A `NeedsRework` verdict from an
/// agent hook is handled separately through the rework gate, not by `OnFailure`.
/// Consumers: `HookConfig`, `actors::guided_plan::actor`.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnFailure {
    /// Halt the plan immediately; no further phases run.
    #[default]
    Stop,
    /// Emit a warning to the TUI output but continue to the next hook.
    Warn,
    /// Silently continue to the next hook regardless of outcome.
    Continue,
}

/// Selects how the verdict is extracted from a Copilot agent hook session.
///
/// `ToolCall` (recommended) waits for the agent to call `approve_phase` or
/// `request_rework` tools. `VerdictSuffix` scans accumulated response text for
/// `VERDICT: PASS` or `VERDICT: REWORK(reason)` patterns.
/// Consumers: `CopilotAgentHookParams`, `actors::guided_plan::hooks::copilot_agent`.
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerdictKind {
    /// The agent calls `approve_phase` or `request_rework` as tool calls.
    #[default]
    ToolCall,
    /// The agent appends `VERDICT: PASS` or `VERDICT: REWORK(reason)` to its response.
    VerdictSuffix,
}

/// Parameters for a subprocess hook: the shell command to execute.
///
/// Consumers: `HookType::Subprocess`, `actors::guided_plan::hooks::subprocess`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SubprocessHookParams {
    /// Shell command string to execute, e.g. `"cargo test domain"`.
    pub command: ShellCommand,
}

/// Parameters for a Copilot agent hook: which agent to invoke and how.
///
/// `agent` is the agent identifier passed to the SDK (e.g. `"code-reviewer"`).
/// `prompt` is the message sent as the first turn of the scoped session.
/// `verdict` determines whether the hook result is extracted via tool calls or
/// text suffix pattern matching.
/// Consumers: `HookType::CopilotAgent`, `actors::guided_plan::hooks::copilot_agent`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CopilotAgentHookParams {
    /// Copilot agent identifier, e.g. `"code-reviewer"`.
    pub agent: AgentName,
    /// Prompt sent as the first message to the scoped agent session.
    pub prompt: PromptText,
    /// How the agent communicates its verdict.
    #[serde(default)]
    pub verdict: VerdictKind,
}

/// Discriminated union of hook types in a post-phase sequence.
///
/// Consumers: `HookConfig`, `actors::guided_plan::actor::run_hooks`.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookType {
    /// Run a shell subprocess and check exit code.
    Subprocess(SubprocessHookParams),
    /// Invoke a scoped Copilot agent session and wait for a verdict.
    CopilotAgent(CopilotAgentHookParams),
}

/// Configuration for a single post-phase hook.
///
/// Specifies what to run (`hook_type`), what to do on infrastructure failure
/// (`on_failure`), and whether this hook should be re-run when a phase enters
/// the rework loop (`rerun_on_rework`).
/// Consumers: `PostPhaseConfig`, `actors::guided_plan::actor`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct HookConfig {
    /// The hook variant and its parameters.
    #[serde(flatten)]
    pub hook_type: HookType,
    /// What to do when the hook itself fails (not a rework verdict).
    #[serde(default)]
    pub on_failure: OnFailure,
    /// Whether this hook is re-run when the phase re-enters the rework loop.
    #[serde(default = "default_true")]
    pub rerun_on_rework: IsPredicate,
}

/// Returns `true`; used as the serde default for `HookConfig::rerun_on_rework`.
fn default_true() -> IsPredicate {
    true.into()
}

// ── Plan structure ────────────────────────────────────────────────────────────

/// Post-phase automated actions run after the user confirms a phase is complete.
///
/// `commit` triggers an automated commit prompt. `compact` triggers conversation
/// compaction and blocks phase advancement until `CompactionDone` is received.
/// `hooks` lists subprocess and agent checks run in order.
/// Consumers: `GuidedPlanPhase`, `actors::guided_plan::actor::run_post_phase`.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct PostPhaseConfig {
    /// When `true`, injects a commit prompt into the main chat after the phase.
    #[serde(default)]
    pub commit: IsPredicate,
    /// When `true`, triggers conversation compaction and blocks until done.
    #[serde(default)]
    pub compact: IsPredicate,
    /// Ordered list of hooks to run after phase work completes.
    #[serde(default)]
    pub hooks: Vec<HookConfig>,
}

/// One phase in a guided plan.
///
/// `id` is the unique phase key used in events and status reporting.
/// `name` is the human-readable display name shown in the TUI panel.
/// `prompt` is an optional instruction injected into the main chat before phase
/// work begins; `None` means no auto-inject.
/// `post_phase` defines automated actions run after the user confirms completion.
/// Consumers: `GuidedPlanConfig`, `actors::guided_plan::actor`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct GuidedPlanPhase {
    /// Unique phase identifier, e.g. `"phase-1"`. Maps to the `id` field in YAML.
    pub id: PlanPhaseId,
    /// Human-readable phase name shown in the TUI right panel.
    pub name: PhaseName,
    /// Optional prompt injected into the main chat when the phase starts.
    pub prompt: Option<PromptText>,
    /// Automated actions run after the user confirms this phase is complete.
    #[serde(default)]
    pub post_phase: PostPhaseConfig,
}

/// Top-level configuration parsed from the YAML frontmatter of a guided plan file.
///
/// Deserialized from YAML by `actors::guided_plan::loader::load_guided_plan`.
/// Consumers: `GuidedPlanActor`, `TUI /run-plan command handler`, `ConversationMode::GuidedPlan`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct GuidedPlanConfig {
    /// Human-readable plan name shown in the TUI panel header.
    pub name: PlanName,
    /// Ordered list of phases. Phases execute sequentially.
    pub phases: Vec<GuidedPlanPhase>,
}

// ── Runtime status types ──────────────────────────────────────────────────────

/// Runtime status of a single phase in the guided plan state machine.
///
/// Transitions: `Pending` → `InProgress` → `AwaitingHooks` → `Complete` or
/// `NeedsRework(reason)`. From `NeedsRework`, the phase returns to `InProgress`
/// when the user re-enters the rework loop. `Failed` is terminal.
/// Consumers: `GuidedPlanRunState`, `GuidedPlanEvent::PhaseStatusChanged`,
/// `GuidedPlanUiState`, `actors::guided_plan::actor`.
#[derive(Clone, Debug, PartialEq)]
pub enum PhaseStatus {
    /// Phase has not been started yet.
    Pending,
    /// Phase is actively being worked on; user has not yet confirmed.
    InProgress,
    /// User confirmed; hooks are running.
    AwaitingHooks,
    /// An agent hook requested rework; holds the reason message.
    NeedsRework(ReworkReason),
    /// All hooks passed; phase is complete.
    Complete,
    /// A hook with `on_failure: Stop` failed; plan is halted.
    Failed(FailureReason),
}

/// Outcome produced by a single hook runner.
///
/// Returned by `run_subprocess_hook` and `run_copilot_agent_hook` and consumed
/// by `actors::guided_plan::actor::run_hooks` to determine gate results.
#[derive(Clone, Debug)]
pub enum HookOutcome {
    /// Hook passed; no rework needed.
    Passed,
    /// Hook failed with a description of what went wrong.
    Failed(FailureReason),
    /// Agent hook requested rework; holds the reviewer's reason.
    NeedsRework(ReworkReason),
    /// Hook was skipped (e.g. `on_failure: Continue` after a prior skip, or
    /// non-`copilot-executor` build for a Copilot agent hook).
    Skipped,
}

// ── Actor events ──────────────────────────────────────────────────────────────

/// Events emitted by `GuidedPlanActor` on its broadcast channel.
///
/// Consumed by the TUI actor to update `ConversationMode::GuidedPlan` state, render
/// reviewer tokens in the main chat, and handle plan lifecycle signals.
/// Consumers: `actors::tui::actor`, `actors::guided_plan::handle`.
#[derive(Clone, Debug)]
pub enum GuidedPlanEvent {
    /// A phase's status changed; the TUI should update the right panel.
    PhaseStatusChanged {
        /// Zero-based index into `GuidedPlanConfig::phases`.
        phase_idx: PhaseIndex,
        /// New status for the phase.
        status: PhaseStatus,
    },
    /// A text token from a Copilot agent hook; the TUI renders it in main chat
    /// with a `"Reviewer: "` prefix on the first token of each reviewer turn.
    ReviewToken(OutputText),
    /// A single line of subprocess hook output.
    HookOutput {
        /// Zero-based phase index.
        phase_idx: PhaseIndex,
        /// Zero-based hook index within the phase's hook list.
        hook_idx: HookIndex,
        /// One captured output line.
        line: OutputText,
    },
    /// All phases reached `Complete`; the TUI shows a success banner.
    PlanComplete,
    /// A hook with `on_failure: Stop` failed; the plan is halted.
    PlanFailed {
        /// Zero-based phase index where the failure occurred.
        phase_idx: PhaseIndex,
        /// Description of what failed.
        reason: FailureReason,
    },
    /// The actor requests that the TUI trigger conversation compaction.
    CompactRequested,
    /// The actor requests that the TUI inject a commit prompt into the main chat.
    CommitRequested,
}

/// Arguments for a copilot-agent hook runner implementation.
#[derive(Clone)]
pub struct CopilotAgentHookArgs {
    /// Hook parameters deserialized from guided-plan frontmatter.
    pub params: CopilotAgentHookParams,
    /// Broadcast sender used to emit review tokens to TUI subscribers.
    pub event_tx: broadcast::Sender<GuidedPlanEvent>,
}

/// Boxed future returned by a copilot-agent hook runner.
pub type CopilotAgentHookFuture = Pin<Box<dyn Future<Output = HookOutcome> + Send + 'static>>;

/// Runtime-injected copilot-agent hook runner.
pub type CopilotAgentHookRunner =
    Arc<dyn Fn(CopilotAgentHookArgs) -> CopilotAgentHookFuture + Send + Sync>;

/// Maximum number of stdout + stderr lines captured from a subprocess hook.
pub const MAX_HOOK_OUTPUT_LINES: usize = 500;

/// Build a default copilot-agent hook runner used when provider wiring is absent.
pub fn unavailable_copilot_hook_runner() -> CopilotAgentHookRunner {
    Arc::new(|_args| {
        Box::pin(async {
            HookOutcome::Failed(FailureReason::from(
                "copilot agent hook runner is not wired",
            ))
        })
    })
}
