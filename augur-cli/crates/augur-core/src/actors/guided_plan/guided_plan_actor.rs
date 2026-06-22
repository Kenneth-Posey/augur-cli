//! Guided plan actor: owns the runtime state machine and drives hook execution.

use super::commands::GuidedPlanCmd;
use super::handle::GuidedPlanHandle;
use super::hooks::subprocess::run_subprocess_hook;
use super::hooks::{unavailable_copilot_hook_runner, CopilotAgentHookArgs, CopilotAgentHookRunner};
use augur_domain::domain::guided_plan::{
    GuidedPlanConfig, GuidedPlanEvent, HookConfig, HookOutcome, HookType, OnFailure, PhaseStatus,
};
use augur_domain::domain::{FailureReason, HookIndex, OutputText, PhaseIndex};
use tokio::sync::{broadcast, mpsc};

/// Command channel capacity for the guided plan actor.
///
/// A small buffer is sufficient because commands arrive from a single UI source
/// at human interaction speed. Consumers: `spawn`.
const GUIDED_PLAN_CMD_CAPACITY: usize = 16;

/// Broadcast channel capacity for guided plan events.
///
/// Large enough to buffer bursts of hook output lines without dropping events
/// when the TUI task is momentarily busy. Consumers: `spawn`.
const GUIDED_PLAN_EVENT_CAPACITY: usize = 256;

/// Runtime state held by the guided plan actor while a plan is executing.
///
/// Owns the current plan config, per-phase statuses, pointer to the active phase,
/// indices of hooks not yet passed for that phase, and the compaction wait flag.
#[derive(bon::Builder)]
struct GuidedPlanRunState {
    /// Parsed plan configuration.
    config: GuidedPlanConfig,
    /// Per-phase runtime status; one entry per `config.phases` element.
    phase_statuses: Vec<PhaseStatus>,
    /// Zero-based index of the phase currently being worked on.
    current_phase: usize,
    /// Indices (into `config.phases[current_phase].post_phase.hooks`) of hooks
    /// that have not yet returned `Passed` for the current phase run.
    pending_hooks: Vec<usize>,
    /// Set to `true` when the actor is waiting for a `CompactionDone` command
    /// before advancing to the next phase.
    awaiting_compact: bool,
}

impl GuidedPlanRunState {
    fn new(config: GuidedPlanConfig) -> Self {
        let count = config.phases.len();
        GuidedPlanRunState::builder()
            .config(config)
            .phase_statuses(vec![PhaseStatus::Pending; count])
            .current_phase(0)
            .pending_hooks(Vec::new())
            .awaiting_compact(false)
            .build()
    }
}

/// Spawn the `GuidedPlanActor` task and return its handle.
///
/// Creates the command mpsc and event broadcast channels, then spawns the
/// `run_loop` task. The returned `GuidedPlanHandle` is the only way to send
/// commands and subscribe to events.
///
/// Consumers: `wiring::run`.
pub fn spawn() -> GuidedPlanHandle {
    spawn_with_copilot_hook_runner(unavailable_copilot_hook_runner())
}

/// Spawn the `GuidedPlanActor` task with a runtime-provided copilot hook runner.
///
/// The core crate stays provider-agnostic by receiving this runner from the
/// composition root instead of importing provider SDK code directly.
pub fn spawn_with_copilot_hook_runner(
    copilot_hook_runner: CopilotAgentHookRunner,
) -> GuidedPlanHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel::<GuidedPlanCmd>(GUIDED_PLAN_CMD_CAPACITY);
    let (event_tx, _) = broadcast::channel::<GuidedPlanEvent>(GUIDED_PLAN_EVENT_CAPACITY);
    let handle = GuidedPlanHandle {
        cmd_tx,
        event_tx: event_tx.clone(),
    };
    tokio::spawn(run_loop(cmd_rx, event_tx, copilot_hook_runner));
    handle
}

/// Main actor loop: receives commands and drives the plan state machine.
/// Handles `GuidedPlanCmd::ConfirmPhase`: runs post-phase hooks and advances on pass.
///
/// When the current phase is `InProgress`, transitions to `AwaitingHooks`, runs all
/// configured hooks, and either advances to the next phase (or waits for compaction)
/// on `AllPassed`, or sets the gate result status on failure/rework.
async fn handle_confirm_phase(s: &mut GuidedPlanRunState, ctx: ConfirmPhaseContext<'_>) {
    let is_in_progress = matches!(s.phase_statuses[s.current_phase], PhaseStatus::InProgress);
    if !is_in_progress {
        return;
    }
    s.phase_statuses[s.current_phase] = PhaseStatus::AwaitingHooks;
    emit(
        ctx.event_tx,
        GuidedPlanEvent::PhaseStatusChanged {
            phase_idx: PhaseIndex::from(s.current_phase),
            status: PhaseStatus::AwaitingHooks,
        },
    );
    let outcomes = run_hooks(s, ctx.event_tx, ctx.copilot_hook_runner).await;
    let after = apply_hook_outcomes(s, outcomes, ctx.event_tx);
    if matches!(after, HookGateResult::AllPassed) {
        s.phase_statuses[s.current_phase] = PhaseStatus::Complete;
        emit(
            ctx.event_tx,
            GuidedPlanEvent::PhaseStatusChanged {
                phase_idx: PhaseIndex::from(s.current_phase),
                status: PhaseStatus::Complete,
            },
        );
        let needs_compact = run_post_phase_commit_compact(s, ctx.event_tx);
        if needs_compact {
            *ctx.compact_advance_pending = true;
        } else {
            advance_to_next_phase(s, ctx.event_tx);
        }
    }
}

struct ConfirmPhaseContext<'a> {
    event_tx: &'a broadcast::Sender<GuidedPlanEvent>,
    compact_advance_pending: &'a mut bool,
    copilot_hook_runner: &'a CopilotAgentHookRunner,
}

/// Handles `GuidedPlanCmd::ForceAdvance`: overrides a `NeedsRework` gate and advances.
///
/// Only acts when the current phase status is `NeedsRework`. Transitions to `Complete`
/// and advances (or waits for compaction), logging a warning for the override.
fn handle_force_advance(
    s: &mut GuidedPlanRunState,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
    compact_advance_pending: &mut bool,
) {
    let is_rework = matches!(
        s.phase_statuses[s.current_phase],
        PhaseStatus::NeedsRework(_)
    );
    if !is_rework {
        return;
    }
    tracing::warn!(
        phase_idx = s.current_phase,
        "ForceAdvance: overriding NeedsRework gate"
    );
    s.phase_statuses[s.current_phase] = PhaseStatus::Complete;
    emit(
        event_tx,
        GuidedPlanEvent::PhaseStatusChanged {
            phase_idx: PhaseIndex::from(s.current_phase),
            status: PhaseStatus::Complete,
        },
    );
    let needs_compact = run_post_phase_commit_compact(s, event_tx);
    if needs_compact {
        *compact_advance_pending = true;
    } else {
        advance_to_next_phase(s, event_tx);
    }
}

/// Handles `GuidedPlanCmd::CompactionDone`: clears the pending flag and advances.
///
/// Only acts when `compact_advance_pending` is set. Resets `awaiting_compact` on
/// the state and calls `advance_to_next_phase`.
fn handle_compaction_done(
    state: &mut Option<GuidedPlanRunState>,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
    compact_advance_pending: &mut bool,
) {
    if !*compact_advance_pending {
        return;
    }
    *compact_advance_pending = false;
    if let Some(s) = state {
        s.awaiting_compact = false;
        advance_to_next_phase(s, event_tx);
    }
}

/// Main actor loop: receives commands and drives the plan state machine.
async fn run_loop(
    mut cmd_rx: mpsc::Receiver<GuidedPlanCmd>,
    event_tx: broadcast::Sender<GuidedPlanEvent>,
    copilot_hook_runner: CopilotAgentHookRunner,
) {
    let mut state: Option<GuidedPlanRunState> = None;
    let mut compact_advance_pending = false;
    let mut ctx = RunLoopCmdContext {
        state: &mut state,
        event_tx: &event_tx,
        compact_advance_pending: &mut compact_advance_pending,
    };

    while let Some(cmd) = cmd_rx.recv().await {
        if handle_run_loop_cmd(cmd, &mut ctx, &copilot_hook_runner)
            .await
            .is_break()
        {
            break;
        }
    }
}

enum RunLoopControl {
    Continue,
    Break,
}

impl RunLoopControl {
    fn is_break(&self) -> bool {
        matches!(self, Self::Break)
    }
}

struct RunLoopCmdContext<'a> {
    state: &'a mut Option<GuidedPlanRunState>,
    event_tx: &'a broadcast::Sender<GuidedPlanEvent>,
    compact_advance_pending: &'a mut bool,
}

async fn handle_run_loop_cmd(
    cmd: GuidedPlanCmd,
    ctx: &mut RunLoopCmdContext<'_>,
    copilot_hook_runner: &CopilotAgentHookRunner,
) -> RunLoopControl {
    if matches!(cmd, GuidedPlanCmd::Shutdown) {
        return RunLoopControl::Break;
    }
    handle_non_shutdown_cmd(cmd, ctx, copilot_hook_runner).await;
    RunLoopControl::Continue
}

async fn handle_non_shutdown_cmd(
    cmd: GuidedPlanCmd,
    ctx: &mut RunLoopCmdContext<'_>,
    copilot_hook_runner: &CopilotAgentHookRunner,
) {
    if let GuidedPlanCmd::Start { config, .. } = cmd {
        *ctx.state = Some(handle_start(config, ctx.event_tx));
        *ctx.compact_advance_pending = false;
        return;
    }
    if matches!(cmd, GuidedPlanCmd::ConfirmPhase) {
        handle_confirm_if_running(ctx, copilot_hook_runner).await;
        return;
    }
    if matches!(cmd, GuidedPlanCmd::ForceAdvance) {
        handle_force_advance_if_running(ctx);
        return;
    }
    if matches!(cmd, GuidedPlanCmd::CompactionDone) {
        handle_compaction_done_if_pending(ctx);
    }
}

async fn handle_confirm_if_running(
    ctx: &mut RunLoopCmdContext<'_>,
    copilot_hook_runner: &CopilotAgentHookRunner,
) {
    if let Some(state) = ctx.state.as_mut() {
        handle_confirm_phase(
            state,
            ConfirmPhaseContext {
                event_tx: ctx.event_tx,
                compact_advance_pending: ctx.compact_advance_pending,
                copilot_hook_runner,
            },
        )
        .await;
    }
}

fn handle_force_advance_if_running(ctx: &mut RunLoopCmdContext<'_>) {
    if let Some(state) = ctx.state.as_mut() {
        handle_force_advance(state, ctx.event_tx, ctx.compact_advance_pending);
    }
}

fn handle_compaction_done_if_pending(ctx: &mut RunLoopCmdContext<'_>) {
    handle_compaction_done(ctx.state, ctx.event_tx, ctx.compact_advance_pending);
}
/// Initialise run state, emit `Pending` for all phases, and set phase 0 to `InProgress`.
fn handle_start(
    config: GuidedPlanConfig,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
) -> GuidedPlanRunState {
    let mut s = GuidedPlanRunState::new(config);
    for i in 0..s.phase_statuses.len() {
        emit(
            event_tx,
            GuidedPlanEvent::PhaseStatusChanged {
                phase_idx: PhaseIndex::from(i),
                status: PhaseStatus::Pending,
            },
        );
    }
    if !s.phase_statuses.is_empty() {
        s.phase_statuses[0] = PhaseStatus::InProgress;
        s.pending_hooks = build_pending_hooks(&s.config, 0, false);
        emit(
            event_tx,
            GuidedPlanEvent::PhaseStatusChanged {
                phase_idx: PhaseIndex::from(0),
                status: PhaseStatus::InProgress,
            },
        );
    }
    s
}

/// Build the list of pending hook indices for a phase.
///
/// When `rework_only` is `true`, only hooks with `rerun_on_rework = true` are
/// included. When `false`, all hook indices are included.
fn build_pending_hooks(
    config: &GuidedPlanConfig,
    phase_idx: usize,
    rework_only: bool,
) -> Vec<usize> {
    let hooks = &config.phases[phase_idx].post_phase.hooks;
    (0..hooks.len())
        .filter(|&i| !rework_only || hooks[i].rerun_on_rework.0)
        .collect()
}

/// Bundles the emission context needed by `run_single_hook`.
///
/// Groups phase and hook indices with the event sender so that `run_single_hook`
/// stays within the 3-parameter limit. Consumers: `run_hooks`.
struct HookEmitCtx<'a> {
    /// Zero-based index of the phase owning this hook.
    phase_idx: usize,
    /// Zero-based index of the hook within the phase's hook list.
    hook_idx: usize,
    /// Broadcast sender for emitting `HookOutput` events.
    event_tx: &'a broadcast::Sender<GuidedPlanEvent>,
    /// Runtime-provided hook runner for `HookType::CopilotAgent`.
    copilot_hook_runner: &'a CopilotAgentHookRunner,
}

/// Run all pending hooks for the current phase, emitting `HookOutput` lines.
async fn run_hooks(
    state: &mut GuidedPlanRunState,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
    copilot_hook_runner: &CopilotAgentHookRunner,
) -> Vec<(usize, HookOutcome)> {
    let phase_idx = state.current_phase;
    let mut outcomes = Vec::with_capacity(state.pending_hooks.len());

    for hook_idx in state.pending_hooks.iter().copied() {
        let hook = &state.config.phases[phase_idx].post_phase.hooks[hook_idx];
        let ctx = HookEmitCtx {
            phase_idx,
            hook_idx,
            event_tx,
            copilot_hook_runner,
        };
        let outcome = run_single_hook(hook, &ctx).await;
        outcomes.push((hook_idx, outcome));
    }
    outcomes
}

/// Dispatch to the appropriate hook runner based on `HookType`.
///
/// Emits `HookOutput` events via `ctx.event_tx` for subprocess failures.
/// Returns the `HookOutcome` from the runner.
async fn run_single_hook(hook: &HookConfig, ctx: &HookEmitCtx<'_>) -> HookOutcome {
    match &hook.hook_type {
        HookType::Subprocess(params) => {
            let outcome = run_subprocess_hook(&params.command).await;
            if matches!(outcome, HookOutcome::Failed(_)) {
                emit(
                    ctx.event_tx,
                    GuidedPlanEvent::HookOutput {
                        phase_idx: PhaseIndex::from(ctx.phase_idx),
                        hook_idx: HookIndex::from(ctx.hook_idx),
                        line: OutputText::from(format!(
                            "[subprocess hook failed: {}]",
                            params.command
                        )),
                    },
                );
            }
            outcome
        }
        HookType::CopilotAgent(params) => {
            let args = CopilotAgentHookArgs {
                params: params.clone(),
                event_tx: ctx.event_tx.clone(),
            };
            (ctx.copilot_hook_runner)(args).await
        }
    }
}

/// Decision produced by `apply_hook_outcomes`.
///
/// Payloads are emitted to the event bus before returning, so the caller only
/// needs to distinguish `AllPassed` from the failure variants.
enum HookGateResult {
    AllPassed,
    NeedsRework,
    Failed,
}

/// Handles a `HookOutcome::Failed` result based on the hook's `OnFailure` policy.
///
/// - `Stop`: marks the phase as `Failed`, emits `PhaseStatusChanged` + `PlanFailed`, returns `Failed`.
/// - `Warn`: logs a warning, retains hook in pending list but continues, returns `None`.
/// - `Continue`: removes hook from pending, returns `None`.
///
/// Returns `Some(HookGateResult)` when the failure halts processing; `None` to continue.
struct HookFailureContext<'a> {
    hook_idx: usize,
    on_failure: &'a OnFailure,
    message: &'a FailureReason,
    event_tx: &'a broadcast::Sender<GuidedPlanEvent>,
}

fn handle_hook_failure(
    state: &mut GuidedPlanRunState,
    failure: HookFailureContext<'_>,
) -> Option<HookGateResult> {
    match failure.on_failure {
        OnFailure::Stop => {
            let reason = FailureReason::from(format!(
                "hook {} failed: {}",
                failure.hook_idx, failure.message
            ));
            state.phase_statuses[state.current_phase] = PhaseStatus::Failed(reason.clone());
            emit(
                failure.event_tx,
                GuidedPlanEvent::PhaseStatusChanged {
                    phase_idx: PhaseIndex::from(state.current_phase),
                    status: PhaseStatus::Failed(reason.clone()),
                },
            );
            emit(
                failure.event_tx,
                GuidedPlanEvent::PlanFailed {
                    phase_idx: PhaseIndex::from(state.current_phase),
                    reason: reason.clone(),
                },
            );
            Some(HookGateResult::Failed)
        }
        OnFailure::Warn => {
            tracing::warn!(
                phase_idx = state.current_phase,
                hook_idx = failure.hook_idx,
                message = %failure.message,
                "hook failed with on_failure=warn; continuing"
            );
            state.pending_hooks.retain(|&i| i != failure.hook_idx);
            None
        }
        OnFailure::Continue => {
            state.pending_hooks.retain(|&i| i != failure.hook_idx);
            None
        }
    }
}

/// Apply hook outcomes to state, returning the aggregate gate result.
///
/// Iterates outcomes in order. The first `NeedsRework` or critical `Failed`
/// determines the overall result. `OnFailure::Warn | Continue` failures are
/// logged but do not halt the sequence.
fn apply_hook_outcomes(
    state: &mut GuidedPlanRunState,
    outcomes: Vec<(usize, HookOutcome)>,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
) -> HookGateResult {
    for (hook_idx, outcome) in outcomes {
        if let Some(result) =
            apply_single_hook_outcome(state, SingleHookOutcome { hook_idx, outcome }, event_tx)
        {
            return result;
        }
    }
    HookGateResult::AllPassed
}

struct SingleHookOutcome {
    hook_idx: usize,
    outcome: HookOutcome,
}

fn apply_single_hook_outcome(
    state: &mut GuidedPlanRunState,
    hook_outcome: SingleHookOutcome,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
) -> Option<HookGateResult> {
    match hook_outcome.outcome {
        HookOutcome::Passed | HookOutcome::Skipped => {
            state.pending_hooks.retain(|&i| i != hook_outcome.hook_idx);
            None
        }
        HookOutcome::NeedsRework(reason) => {
            state.pending_hooks.retain(|&i| i != hook_outcome.hook_idx);
            state.phase_statuses[state.current_phase] = PhaseStatus::NeedsRework(reason.clone());
            emit(
                event_tx,
                GuidedPlanEvent::PhaseStatusChanged {
                    phase_idx: PhaseIndex::from(state.current_phase),
                    status: PhaseStatus::NeedsRework(reason),
                },
            );
            Some(HookGateResult::NeedsRework)
        }
        HookOutcome::Failed(msg) => {
            let on_failure = hook_on_failure(state, hook_outcome.hook_idx);
            handle_hook_failure(
                state,
                HookFailureContext {
                    hook_idx: hook_outcome.hook_idx,
                    on_failure: &on_failure,
                    message: &msg,
                    event_tx,
                },
            )
        }
    }
}

fn hook_on_failure(state: &GuidedPlanRunState, hook_idx: usize) -> OnFailure {
    state.config.phases[state.current_phase]
        .post_phase
        .hooks
        .get(hook_idx)
        .map(|hook| hook.on_failure.clone())
        .unwrap_or(OnFailure::Stop)
}

/// Emit the commit and compact events for the current phase's post-phase config.
///
/// Returns `true` when a compaction was requested (the caller should set
/// `compact_advance_pending = true` and wait for `CompactionDone`).
fn run_post_phase_commit_compact(
    state: &mut GuidedPlanRunState,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
) -> bool {
    let post = &state.config.phases[state.current_phase].post_phase;
    if post.commit.0 {
        emit(event_tx, GuidedPlanEvent::CommitRequested);
    }
    if post.compact.0 {
        state.awaiting_compact = true;
        emit(event_tx, GuidedPlanEvent::CompactRequested);
        return true;
    }
    false
}

/// Advance `current_phase` to the next phase, or emit `PlanComplete`.
fn advance_to_next_phase(
    state: &mut GuidedPlanRunState,
    event_tx: &broadcast::Sender<GuidedPlanEvent>,
) {
    let next = state.current_phase + 1;
    if next >= state.config.phases.len() {
        emit(event_tx, GuidedPlanEvent::PlanComplete);
        return;
    }
    state.current_phase = next;
    state.phase_statuses[next] = PhaseStatus::InProgress;
    state.pending_hooks = build_pending_hooks(&state.config, next, false);
    emit(
        event_tx,
        GuidedPlanEvent::PhaseStatusChanged {
            phase_idx: PhaseIndex::from(next),
            status: PhaseStatus::InProgress,
        },
    );
}

/// Send an event on the broadcast channel, ignoring errors when no receivers are connected.
fn emit(tx: &broadcast::Sender<GuidedPlanEvent>, event: GuidedPlanEvent) {
    let _ = tx.send(event);
}

#[cfg(test)]
mod tests {
    use super::{spawn, spawn_with_copilot_hook_runner};
    use crate::actors::guided_plan::hooks::CopilotAgentHookRunner;
    use augur_domain::domain::guided_plan::{
        CopilotAgentHookParams, GuidedPlanConfig, GuidedPlanEvent, GuidedPlanPhase, HookConfig,
        HookOutcome, HookType, OnFailure, PostPhaseConfig, VerdictKind,
    };
    use augur_domain::domain::StringNewtype;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn guided_plan_config_for_agent(agent: &str) -> GuidedPlanConfig {
        GuidedPlanConfig {
            name: "test-plan".into(),
            phases: vec![GuidedPlanPhase {
                id: "phase-1".into(),
                name: "Phase 1".into(),
                prompt: None,
                post_phase: PostPhaseConfig {
                    hooks: vec![HookConfig {
                        hook_type: HookType::CopilotAgent(CopilotAgentHookParams {
                            agent: agent.into(),
                            prompt: "verify this phase".into(),
                            verdict: VerdictKind::ToolCall,
                        }),
                        on_failure: OnFailure::Stop,
                        rerun_on_rework: true.into(),
                    }],
                    ..PostPhaseConfig::default()
                },
            }],
        }
    }

    async fn collect_events_until_terminal(
        rx: &mut tokio::sync::broadcast::Receiver<GuidedPlanEvent>,
    ) -> Vec<GuidedPlanEvent> {
        let mut events = Vec::new();
        for _ in 0..16 {
            let recv = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
            let Ok(Ok(event)) = recv else {
                break;
            };
            let is_terminal = matches!(
                event,
                GuidedPlanEvent::PlanComplete | GuidedPlanEvent::PlanFailed { .. }
            );
            events.push(event);
            if is_terminal {
                break;
            }
        }
        events
    }

    #[tokio::test]
    async fn injected_copilot_runner_path_is_used() {
        let invoked = Arc::new(AtomicBool::new(false));
        let marker = Arc::clone(&invoked);
        let runner: CopilotAgentHookRunner = Arc::new(move |_args| {
            let called = Arc::clone(&marker);
            Box::pin(async move {
                called.store(true, Ordering::SeqCst);
                HookOutcome::Passed
            })
        });

        let handle = spawn_with_copilot_hook_runner(runner);
        let mut rx = handle.subscribe();
        handle.start(
            guided_plan_config_for_agent("test-agent"),
            "plans/test.md".into(),
        );
        handle.confirm_phase();
        let events = collect_events_until_terminal(&mut rx).await;
        handle.shutdown();

        assert!(invoked.load(Ordering::SeqCst));
        assert!(events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanComplete)));
        assert!(!events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanFailed { .. })));
    }

    #[tokio::test]
    async fn no_wiring_path_fails_copilot_hook_without_skip() {
        let handle = spawn();
        let mut rx = handle.subscribe();
        handle.start(
            guided_plan_config_for_agent("test-agent"),
            "plans/test.md".into(),
        );
        handle.confirm_phase();
        let events = collect_events_until_terminal(&mut rx).await;
        handle.shutdown();

        assert!(events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanFailed { .. })));
        assert!(!events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanComplete)));
        let failure_reason = events.iter().find_map(|event| match event {
            GuidedPlanEvent::PlanFailed { reason, .. } => Some(reason.as_str().to_owned()),
            _ => None,
        });
        assert!(failure_reason.is_some());
        assert!(failure_reason.unwrap_or_default().contains("not wired"));
    }
}
