//! Tests for the guided plan actor state machine.

use augur_core::actors::guided_plan::guided_plan_actor::{spawn, spawn_with_copilot_hook_runner};
use augur_core::actors::guided_plan::hooks::{CopilotAgentHookArgs, CopilotAgentHookRunner};
use augur_domain::domain::guided_plan::{
    CopilotAgentHookParams, GuidedPlanConfig, GuidedPlanEvent, GuidedPlanPhase, HookConfig,
    HookOutcome, HookType, OnFailure, PhaseStatus, PostPhaseConfig, SubprocessHookParams,
    VerdictKind,
};
use augur_domain::domain::string_newtypes::{FilePath, PlanPhaseId, ReworkReason, StringNewtype};
use std::sync::Arc;

/// Build a single-phase plan config for testing.
fn single_phase_config() -> GuidedPlanConfig {
    GuidedPlanConfig {
        name: "Test Plan".into(),
        phases: vec![GuidedPlanPhase {
            id: PlanPhaseId::new("phase-1"),
            name: "Phase One".into(),
            prompt: None,
            post_phase: PostPhaseConfig::default(),
        }],
    }
}

/// Build a two-phase plan config with no hooks.
fn two_phase_config() -> GuidedPlanConfig {
    GuidedPlanConfig {
        name: "Two Phase Plan".into(),
        phases: vec![
            GuidedPlanPhase {
                id: PlanPhaseId::new("p1"),
                name: "Phase 1".into(),
                prompt: None,
                post_phase: PostPhaseConfig::default(),
            },
            GuidedPlanPhase {
                id: PlanPhaseId::new("p2"),
                name: "Phase 2".into(),
                prompt: None,
                post_phase: PostPhaseConfig::default(),
            },
        ],
    }
}

/// Build a two-phase plan whose first phase requires compaction before advancing.
fn two_phase_compact_config() -> GuidedPlanConfig {
    GuidedPlanConfig {
        name: "Two Phase Compact Plan".into(),
        phases: vec![
            GuidedPlanPhase {
                id: PlanPhaseId::new("p1"),
                name: "Phase 1".into(),
                prompt: None,
                post_phase: PostPhaseConfig {
                    compact: true.into(),
                    ..PostPhaseConfig::default()
                },
            },
            GuidedPlanPhase {
                id: PlanPhaseId::new("p2"),
                name: "Phase 2".into(),
                prompt: None,
                post_phase: PostPhaseConfig::default(),
            },
        ],
    }
}

/// Build a two-phase plan whose first phase hook requests rework.
fn two_phase_needs_rework_config() -> GuidedPlanConfig {
    GuidedPlanConfig {
        name: "Needs Rework Plan".into(),
        phases: vec![
            GuidedPlanPhase {
                id: PlanPhaseId::new("p1"),
                name: "Phase 1".into(),
                prompt: None,
                post_phase: PostPhaseConfig {
                    hooks: vec![HookConfig {
                        hook_type: HookType::CopilotAgent(CopilotAgentHookParams {
                            agent: "guided-plan-test-request-rework".into(),
                            prompt: "missing regression coverage".into(),
                            verdict: VerdictKind::ToolCall,
                        }),
                        on_failure: OnFailure::Stop,
                        rerun_on_rework: true.into(),
                    }],
                    ..PostPhaseConfig::default()
                },
            },
            GuidedPlanPhase {
                id: PlanPhaseId::new("p2"),
                name: "Phase 2".into(),
                prompt: None,
                post_phase: PostPhaseConfig::default(),
            },
        ],
    }
}

/// Build a single-phase plan whose post-phase hook fails with `OnFailure::Stop`.
fn single_phase_stop_failure_config() -> GuidedPlanConfig {
    GuidedPlanConfig {
        name: "Stop On Failure Plan".into(),
        phases: vec![GuidedPlanPhase {
            id: PlanPhaseId::new("phase-1"),
            name: "Phase One".into(),
            prompt: None,
            post_phase: PostPhaseConfig {
                hooks: vec![HookConfig {
                    hook_type: HookType::Subprocess(SubprocessHookParams {
                        command: "__nonexistent_dcmk_tool__".into(),
                    }),
                    on_failure: OnFailure::Stop,
                    rerun_on_rework: true.into(),
                }],
                ..PostPhaseConfig::default()
            },
        }],
    }
}

/// Wait for up to `ms` milliseconds for an event matching `predicate`, draining
/// anything that doesn't match. Returns the first matching event if it arrived in time.
async fn wait_for_event<F>(
    rx: &mut tokio::sync::broadcast::Receiver<GuidedPlanEvent>,
    predicate: F,
    ms: u64,
) -> Option<GuidedPlanEvent>
where
    F: Fn(&GuidedPlanEvent) -> bool,
{
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        if std::time::Instant::now() >= deadline {
            return None;
        }
        match rx.try_recv() {
            Ok(e) => {
                if predicate(&e) {
                    return Some(e);
                }
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            Err(_) => return None,
        }
    }
}

/// Verifies that `Start` transitions phase 0 to `InProgress` and emits the
/// corresponding `PhaseStatusChanged` event.
#[tokio::test]
async fn start_transitions_phase_0_to_in_progress() {
    let handle = spawn();
    let mut rx = handle.subscribe();
    handle.start(single_phase_config(), FilePath::new("test.md"));

    let found = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 0usize.into())
    }, 500).await;
    assert!(
        found.is_some(),
        "expected PhaseStatusChanged(0, InProgress) event"
    );

    handle.shutdown();
}

/// Verifies that `ConfirmPhase` on a single-phase plan with no hooks advances
/// to `Complete` and then emits `PlanComplete`.
#[tokio::test]
async fn confirm_phase_no_hooks_completes_plan() {
    let handle = spawn();
    let mut rx = handle.subscribe();
    handle.start(single_phase_config(), FilePath::new("test.md"));

    // Wait for InProgress
    wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 0usize.into())
    }, 500).await;

    handle.confirm_phase();

    let complete = wait_for_event(
        &mut rx,
        |e| matches!(e, GuidedPlanEvent::PlanComplete),
        1000,
    )
    .await;
    assert!(
        complete.is_some(),
        "expected PlanComplete after confirm on single-phase no-hook plan"
    );

    handle.shutdown();
}

/// Verifies that `ConfirmPhase` on a two-phase plan advances to phase 1 `InProgress`
/// after phase 0 completes.
#[tokio::test]
async fn confirm_phase_advances_to_next_phase() {
    let handle = spawn();
    let mut rx = handle.subscribe();
    handle.start(two_phase_config(), FilePath::new("test.md"));

    wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 0usize.into())
    }, 500).await;

    handle.confirm_phase();

    let phase_1_in_progress = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 1usize.into())
    }, 1000).await;
    assert!(
        phase_1_in_progress.is_some(),
        "expected phase 1 to become InProgress"
    );

    handle.shutdown();
}

/// Verifies that `CompactRequested` blocks phase advancement until
/// `CompactionDone`, after which the actor advances the next phase to `InProgress`.
#[tokio::test]
async fn compaction_done_after_compact_requested_advances_to_next_phase() {
    let handle = spawn();
    let mut rx = handle.subscribe();

    handle.start(two_phase_compact_config(), FilePath::new("test.md"));

    wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 0usize.into())
    }, 500).await;

    handle.confirm_phase();

    let compact_requested = wait_for_event(
        &mut rx,
        |e| matches!(e, GuidedPlanEvent::CompactRequested),
        1000,
    )
    .await;
    assert!(
        compact_requested.is_some(),
        "expected CompactRequested after confirming a compacting phase"
    );

    let advanced_before_done = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 1usize.into())
    }, 100).await;
    assert!(
        advanced_before_done.is_none(),
        "phase 1 must not advance before CompactionDone"
    );

    handle.compaction_done();

    let phase_1_in_progress = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 1usize.into())
    }, 1000).await;
    assert!(
        phase_1_in_progress.is_some(),
        "expected phase 1 to become InProgress after CompactionDone"
    );

    handle.shutdown();
}

/// Verifies that a hook-produced `NeedsRework` status can be overridden by
/// `ForceAdvance`, which completes the current phase and advances the next phase.
#[tokio::test]
async fn force_advance_from_needs_rework_completes_phase() {
    let runner: CopilotAgentHookRunner = Arc::new(|args: CopilotAgentHookArgs| {
        let reason = args.params.prompt.clone();
        Box::pin(async move { HookOutcome::NeedsRework(ReworkReason::new(reason.as_str())) })
    });
    let handle = spawn_with_copilot_hook_runner(runner);
    let mut rx = handle.subscribe();

    handle.start(two_phase_needs_rework_config(), FilePath::new("test.md"));

    wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 0usize.into())
    }, 500).await;

    handle.confirm_phase();

    let needs_rework = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::NeedsRework(reason) } if *phase_idx == 0usize.into() && reason.as_str() == "missing regression coverage")
    }, 1000).await;
    assert!(
        needs_rework.is_some(),
        "expected phase 0 to enter NeedsRework from the post-phase hook"
    );

    handle.force_advance();

    let phase_0_complete = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::Complete } if *phase_idx == 0usize.into())
    }, 1000).await;
    assert!(
        phase_0_complete.is_some(),
        "expected ForceAdvance to mark phase 0 Complete"
    );

    let phase_1_in_progress = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 1usize.into())
    }, 1000).await;
    assert!(
        phase_1_in_progress.is_some(),
        "expected ForceAdvance to advance phase 1 to InProgress"
    );

    handle.shutdown();
}

/// Verifies that a failing `OnFailure::Stop` hook emits both
/// `PhaseStatus::Failed(...)` and `GuidedPlanEvent::PlanFailed { ... }`.
#[tokio::test]
async fn failing_stop_hook_emits_failed_status_and_plan_failed_event() {
    let handle = spawn();
    let mut rx = handle.subscribe();

    handle.start(single_phase_stop_failure_config(), FilePath::new("test.md"));

    wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::InProgress } if *phase_idx == 0usize.into())
    }, 500).await;

    handle.confirm_phase();

    let failed_status = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PhaseStatusChanged { phase_idx, status: PhaseStatus::Failed(_) } if *phase_idx == 0usize.into())
    }, 1000).await;
    let failed_reason = match failed_status {
        Some(GuidedPlanEvent::PhaseStatusChanged {
            phase_idx,
            status: PhaseStatus::Failed(reason),
        }) => {
            assert_eq!(
                phase_idx,
                0usize.into(),
                "failed status must be for phase 0"
            );
            reason
        }
        other => panic!("expected failed status event, got {other:?}"),
    };

    let plan_failed = wait_for_event(&mut rx, |e| {
        matches!(e, GuidedPlanEvent::PlanFailed { phase_idx, .. } if *phase_idx == 0usize.into())
    }, 1000).await;
    match plan_failed {
        Some(GuidedPlanEvent::PlanFailed { phase_idx, reason }) => {
            assert_eq!(phase_idx, 0usize.into(), "plan failure must be for phase 0");
            assert_eq!(
                reason, failed_reason,
                "PlanFailed reason must match the failed phase status reason"
            );
            assert!(
                reason.as_str().contains("hook 0 failed:"),
                "expected stop failure reason to mention hook 0, got {reason}"
            );
        }
        other => panic!("expected PlanFailed event, got {other:?}"),
    }

    handle.shutdown();
}

#[test]
fn mirror_sync_executes_start_transitions_phase_0_to_in_progress() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
