use augur_cli::wiring::{spawn_agent_runtime, spawn_domain_actors, spawn_planning_actors};
use augur_domain::domain::guided_plan::{
    CopilotAgentHookParams, GuidedPlanConfig, GuidedPlanEvent, GuidedPlanPhase, HookConfig,
    HookType, OnFailure, PostPhaseConfig, VerdictKind,
};
use std::time::Duration;

/// Verifies the mirrored unit-test module can reach this file's surface symbols.
#[test]
fn mirrored_surface_smoke_domain() {
    let function_name = core::any::type_name_of_val(&spawn_domain_actors);
    assert!(function_name.contains("spawn_domain_actors"));
    let function_name = core::any::type_name_of_val(&spawn_planning_actors);
    assert!(function_name.contains("spawn_planning_actors"));
    let function_name = core::any::type_name_of_val(&spawn_agent_runtime);
    assert!(function_name.contains("spawn_agent_runtime"));
}

fn copilot_guided_plan_config() -> GuidedPlanConfig {
    GuidedPlanConfig {
        name: "copilot-guided-plan".into(),
        phases: vec![GuidedPlanPhase {
            id: "phase-1".into(),
            name: "Phase 1".into(),
            prompt: None,
            post_phase: PostPhaseConfig {
                hooks: vec![HookConfig {
                    hook_type: HookType::CopilotAgent(CopilotAgentHookParams {
                        agent: "guided-plan-test-approve".into(),
                        prompt: "approve this phase".into(),
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

#[tokio::test]
async fn startup_injects_copilot_hook_runner_for_guided_plan() {
    let actors = spawn_planning_actors();
    actors.file_scanner.handle.shutdown();
    let handle = actors.guided_plan;
    let mut rx = handle.subscribe();
    handle.start(copilot_guided_plan_config(), "plans/test.md".into());
    handle.confirm_phase();

    let mut saw_complete = false;
    for _ in 0..16 {
        let recv = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        let Ok(Ok(event)) = recv else {
            break;
        };
        if matches!(event, GuidedPlanEvent::PlanComplete) {
            saw_complete = true;
            break;
        }
        if matches!(event, GuidedPlanEvent::PlanFailed { .. }) {
            break;
        }
    }
    handle.shutdown();
    assert!(saw_complete);
}
