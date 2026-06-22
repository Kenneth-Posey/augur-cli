use augur_core::actors::guided_plan::guided_plan_actor::{spawn, spawn_with_copilot_hook_runner};
use augur_core::actors::guided_plan::hooks::CopilotAgentHookRunner;
use augur_domain::domain::guided_plan::{
    CopilotAgentHookParams, GuidedPlanConfig, GuidedPlanEvent, GuidedPlanPhase, HookConfig,
    HookOutcome, HookType, OnFailure, PostPhaseConfig, VerdictKind,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn guided_plan_config_for_agent(agent: &str) -> GuidedPlanConfig {
    GuidedPlanConfig {
        name: "crate-abstraction-behavior-test".into(),
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
async fn gwt_b1_core_guided_plan_hook_runner_is_runtime_injected() {
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
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanComplete))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanFailed { .. }))
    );
}

#[tokio::test]
async fn gwt_b2_unwired_copilot_hook_fails_without_skip() {
    let handle = spawn();
    let mut rx = handle.subscribe();
    handle.start(
        guided_plan_config_for_agent("test-agent"),
        "plans/test.md".into(),
    );
    handle.confirm_phase();
    let events = collect_events_until_terminal(&mut rx).await;
    handle.shutdown();

    assert!(
        events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanFailed { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanComplete))
    );
    let failure_reason = events.iter().find_map(|event| match event {
        GuidedPlanEvent::PlanFailed { reason, .. } => Some(reason.to_string()),
        _ => None,
    });
    assert!(failure_reason.is_some());
    assert!(failure_reason.unwrap_or_default().contains("not wired"));
}

#[tokio::test]
async fn gwt_b3_provider_crate_copilot_hook_runner_is_wired_into_core() {
    let runner = augur_provider_copilot_sdk::guided_plan::hooks::build_copilot_hook_runner();
    let handle = spawn_with_copilot_hook_runner(runner);
    let mut rx = handle.subscribe();
    handle.start(
        guided_plan_config_for_agent("guided-plan-test-approve"),
        "plans/test.md".into(),
    );
    handle.confirm_phase();
    let events = collect_events_until_terminal(&mut rx).await;
    handle.shutdown();

    assert!(
        events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanComplete))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GuidedPlanEvent::PlanFailed { .. }))
    );
}
