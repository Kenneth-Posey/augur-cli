use augur_domain::CopilotAgentHookArgs;
use augur_domain::guided_plan::{CopilotAgentHookParams, HookOutcome, VerdictKind};

fn approve_args() -> CopilotAgentHookArgs {
    let (event_tx, _event_rx) = tokio::sync::broadcast::channel(8);
    CopilotAgentHookArgs {
        params: CopilotAgentHookParams {
            agent: "guided-plan-test-approve".into(),
            prompt: "approve".into(),
            verdict: VerdictKind::ToolCall,
        },
        event_tx,
    }
}

#[tokio::test]
async fn guided_plan_commands_runner_passes_for_test_approve_agent() {
    let runner = augur_provider_copilot_sdk::guided_plan::hooks::build_copilot_hook_runner();
    let outcome = runner(approve_args()).await;
    assert!(matches!(outcome, HookOutcome::Passed));
}
