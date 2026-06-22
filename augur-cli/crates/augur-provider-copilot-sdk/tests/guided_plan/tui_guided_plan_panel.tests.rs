use augur_domain::CopilotAgentHookArgs;
use augur_domain::guided_plan::{CopilotAgentHookParams, HookOutcome, VerdictKind};

#[tokio::test]
async fn guided_plan_panel_hook_runner_is_deterministic_for_test_agent() {
    let (event_tx, _event_rx) = tokio::sync::broadcast::channel(8);
    let args = CopilotAgentHookArgs {
        params: CopilotAgentHookParams {
            agent: "guided-plan-test-approve".into(),
            prompt: "approve panel refresh".into(),
            verdict: VerdictKind::ToolCall,
        },
        event_tx,
    };
    let outcome =
        augur_provider_copilot_sdk::guided_plan::hooks::copilot_agent::run_copilot_agent_hook(args)
            .await;
    assert!(matches!(outcome, HookOutcome::Passed));
}
