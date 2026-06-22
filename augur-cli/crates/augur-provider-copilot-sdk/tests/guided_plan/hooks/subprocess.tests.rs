use augur_domain::CopilotAgentHookArgs;
use augur_domain::guided_plan::{CopilotAgentHookParams, HookOutcome, VerdictKind};

#[tokio::test]
async fn subprocess_hook_path_is_replaced_by_test_override_agents() {
    let (event_tx, _event_rx) = tokio::sync::broadcast::channel(8);
    let args = CopilotAgentHookArgs {
        params: CopilotAgentHookParams {
            agent: "guided-plan-test-request-rework".into(),
            prompt: "subprocess replacement proof".into(),
            verdict: VerdictKind::ToolCall,
        },
        event_tx,
    };
    let outcome =
        augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::run_copilot_agent_hook(args)
            .await;
    assert!(matches!(outcome, HookOutcome::NeedsRework(_)));
}
