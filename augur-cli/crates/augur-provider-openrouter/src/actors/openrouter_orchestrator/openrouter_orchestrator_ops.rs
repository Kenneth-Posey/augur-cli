//! Pure state transitions for OpenRouter orchestrator task-run lifecycle.

use augur_domain::task_types::{
    AwaitRunResult, TaskRunId, TaskRunLifecycleState, TaskRunStatusEntry, TaskRunStatusSnapshot,
    TaskSignal,
};
use std::collections::{HashMap, HashSet};

/// In-memory lifecycle ledger owned by the orchestrator actor.
#[derive(Default, bon::Builder)]
pub struct RunLifecycleLedger {
    /// Run ids accepted but not yet transitioned to active execution.
    pub pending_runs: HashSet<TaskRunId>,
    /// Run ids currently executing.
    pub active_runs: HashSet<TaskRunId>,
    /// Terminal outcomes keyed by run id.
    pub terminal_results: HashMap<TaskRunId, TaskSignal>,
    /// Run ids whose terminal payload has been consumed via await.
    pub consumed_runs: HashSet<TaskRunId>,
}

/// Transition arguments for a pending run entering active execution.
pub struct TransitionToActive {
    /// Correlation id for the run to transition.
    pub run_id: TaskRunId,
}

/// Outcome of attempting to transition a run to active execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionToActiveOutcome {
    /// The run existed in pending and was moved to active.
    MovedFromPending,
    /// The run was not pending; active membership was still enforced.
    MarkedActiveWithoutPendingEntry,
}

/// Deterministically move a run from pending to active.
/// Returns a semantic outcome describing whether pending membership existed.
pub fn transition_to_active(
    ledger: &mut RunLifecycleLedger,
    transition: TransitionToActive,
) -> TransitionToActiveOutcome {
    let was_pending = ledger.pending_runs.remove(&transition.run_id);
    ledger.active_runs.insert(transition.run_id);
    if was_pending {
        TransitionToActiveOutcome::MovedFromPending
    } else {
        TransitionToActiveOutcome::MarkedActiveWithoutPendingEntry
    }
}

/// Terminal result payload for a correlated task run.
pub struct TerminalResultRecord {
    /// Correlation id for the completed/failed/cancelled run.
    pub run_id: TaskRunId,
    /// Terminal signal produced by the run.
    pub signal: TaskSignal,
}

/// Outcome of recording terminal state for a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordTerminalResultOutcome {
    /// The run was known as pending or active before terminalization.
    RecordedFromKnownRun,
    /// The run had no pending/active membership when terminalized.
    RecordedFromUnknownRun,
}

/// Record terminal state and remove the run from pending/active sets.
/// Returns a semantic outcome describing whether the run was previously known.
pub fn record_terminal_result(
    ledger: &mut RunLifecycleLedger,
    record: TerminalResultRecord,
) -> RecordTerminalResultOutcome {
    let removed_pending = ledger.pending_runs.remove(&record.run_id);
    let removed_active = ledger.active_runs.remove(&record.run_id);
    ledger.consumed_runs.remove(&record.run_id);
    ledger.terminal_results.insert(record.run_id, record.signal);
    if removed_pending || removed_active {
        RecordTerminalResultOutcome::RecordedFromKnownRun
    } else {
        RecordTerminalResultOutcome::RecordedFromUnknownRun
    }
}

/// Consume one run's terminal payload with idempotent repeat semantics.
pub fn consume_terminal_result(
    ledger: &mut RunLifecycleLedger,
    run_id: TaskRunId,
) -> AwaitRunResult {
    if let Some(signal) = ledger.terminal_results.remove(&run_id) {
        ledger.consumed_runs.insert(run_id.clone());
        return AwaitRunResult::ConsumedTerminal { run_id, signal };
    }
    if ledger.consumed_runs.contains(&run_id) {
        return AwaitRunResult::AlreadyConsumed { run_id };
    }
    AwaitRunResult::UnknownRun { run_id }
}

/// Resolve a run state without mutating terminal-consumption state.
pub fn resolve_run_state(
    ledger: &RunLifecycleLedger,
    run_id: &TaskRunId,
) -> Option<TaskRunLifecycleState> {
    if ledger.pending_runs.contains(run_id) {
        return Some(TaskRunLifecycleState::Pending);
    }
    if ledger.active_runs.contains(run_id) {
        return Some(TaskRunLifecycleState::Active);
    }
    if let Some(signal) = ledger.terminal_results.get(run_id) {
        return Some(TaskRunLifecycleState::TerminalReady {
            signal: signal.clone(),
        });
    }
    if ledger.consumed_runs.contains(run_id) {
        return Some(TaskRunLifecycleState::TerminalConsumed);
    }
    None
}

/// Build a deterministic, sorted status snapshot for all known runs.
pub struct StatusSnapshotInput {
    /// Maximum number of task workers that may execute in parallel.
    pub max_parallel_workers: usize,
    /// Number of queued runs waiting for worker capacity.
    pub queued_runs: usize,
}

/// Build a deterministic, sorted status snapshot for all known runs.
pub fn status_snapshot(
    ledger: &RunLifecycleLedger,
    input: StatusSnapshotInput,
) -> TaskRunStatusSnapshot {
    let mut known = HashSet::<TaskRunId>::new();
    known.extend(ledger.pending_runs.iter().cloned());
    known.extend(ledger.active_runs.iter().cloned());
    known.extend(ledger.terminal_results.keys().cloned());
    known.extend(ledger.consumed_runs.iter().cloned());
    let mut run_ids = known.into_iter().collect::<Vec<_>>();
    run_ids.sort_by(|left, right| left.as_ref().cmp(right.as_ref()));
    let runs = run_ids
        .into_iter()
        .filter_map(|run_id| {
            resolve_run_state(ledger, &run_id).map(|state| {
                TaskRunStatusEntry::builder()
                    .run_id(run_id)
                    .state(state)
                    .build()
            })
        })
        .collect::<Vec<_>>();
    TaskRunStatusSnapshot::builder()
        .max_parallel_workers(input.max_parallel_workers)
        .active_runs(ledger.active_runs.len())
        .queued_runs(input.queued_runs)
        .terminal_ready_runs(ledger.terminal_results.len())
        .runs(runs)
        .build()
}
