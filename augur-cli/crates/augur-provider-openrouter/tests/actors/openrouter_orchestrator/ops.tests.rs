use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::task_types::{AwaitRunResult, TaskRunId, TaskRunLifecycleState, TaskSignal};
use augur_provider_openrouter::actors::openrouter_orchestrator::openrouter_orchestrator_ops::{
    consume_terminal_result, record_terminal_result, resolve_run_state, status_snapshot,
    transition_to_active, RecordTerminalResultOutcome, RunLifecycleLedger, StatusSnapshotInput,
    TerminalResultRecord, TransitionToActive, TransitionToActiveOutcome,
};

#[test]
fn transition_to_active_moves_run_from_pending_to_active() {
    let run_id = TaskRunId::new("run-1");
    let mut ledger = RunLifecycleLedger::default();
    ledger.pending_runs.insert(run_id.clone());

    let outcome = transition_to_active(
        &mut ledger,
        TransitionToActive {
            run_id: run_id.clone(),
        },
    );

    assert!(matches!(
        outcome,
        TransitionToActiveOutcome::MovedFromPending
    ));
    assert!(!ledger.pending_runs.contains(&run_id));
    assert!(ledger.active_runs.contains(&run_id));
}

#[test]
fn record_then_consume_terminal_result_is_idempotent() {
    let run_id = TaskRunId::new("run-2");
    let mut ledger = RunLifecycleLedger::default();
    ledger.active_runs.insert(run_id.clone());
    let signal = TaskSignal::Failed {
        reason: OutputText::new("boom"),
    };

    let outcome = record_terminal_result(
        &mut ledger,
        TerminalResultRecord {
            run_id: run_id.clone(),
            signal: signal.clone(),
        },
    );
    assert!(matches!(
        outcome,
        RecordTerminalResultOutcome::RecordedFromKnownRun
    ));

    let first = consume_terminal_result(&mut ledger, run_id.clone());
    assert!(matches!(
        first,
        AwaitRunResult::ConsumedTerminal {
            run_id: _,
            signal: TaskSignal::Failed { .. }
        }
    ));
    let second = consume_terminal_result(&mut ledger, run_id.clone());
    assert!(matches!(second, AwaitRunResult::AlreadyConsumed { .. }));
    assert!(matches!(
        resolve_run_state(&ledger, &run_id),
        Some(TaskRunLifecycleState::TerminalConsumed)
    ));
}

#[test]
fn status_snapshot_returns_sorted_run_ids_and_counts() {
    let mut ledger = RunLifecycleLedger::default();
    let run_pending = TaskRunId::new("run-b");
    let run_active = TaskRunId::new("run-a");
    let run_terminal = TaskRunId::new("run-c");
    let run_consumed = TaskRunId::new("run-d");

    ledger.pending_runs.insert(run_pending.clone());
    ledger.active_runs.insert(run_active.clone());
    ledger.terminal_results.insert(
        run_terminal.clone(),
        TaskSignal::Completed {
            output: "done".into(),
        },
    );
    ledger.consumed_runs.insert(run_consumed.clone());

    let snapshot = status_snapshot(
        &ledger,
        StatusSnapshotInput {
            max_parallel_workers: 3,
            queued_runs: 1,
        },
    );

    assert_eq!(snapshot.max_parallel_workers, 3);
    assert_eq!(snapshot.active_runs, 1);
    assert_eq!(snapshot.queued_runs, 1);
    assert_eq!(snapshot.terminal_ready_runs, 1);
    let run_ids = snapshot
        .runs
        .iter()
        .map(|entry| entry.run_id.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(run_ids, vec!["run-a", "run-b", "run-c", "run-d"]);
}
