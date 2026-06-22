use augur_domain::CopilotAgentHookArgs;
use augur_domain::guided_plan::{CopilotAgentHookParams, HookOutcome, VerdictKind};

#[tokio::test]
async fn guided_plan_actor_runner_returns_needs_rework_for_test_rework_agent() {
    let (event_tx, _event_rx) = tokio::sync::broadcast::channel(8);
    let args = CopilotAgentHookArgs {
        params: CopilotAgentHookParams {
            agent: "guided-plan-test-request-rework".into(),
            prompt: "address gap in behavior mapping".into(),
            verdict: VerdictKind::ToolCall,
        },
        event_tx,
    };

    let outcome =
        augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::run_copilot_agent_hook(args)
            .await;
    match outcome {
        HookOutcome::NeedsRework(reason) => {
            assert!(
                reason
                    .to_string()
                    .contains("address gap in behavior mapping")
            );
        }
        other => panic!("expected NeedsRework outcome, got {other:?}"),
    }
}
