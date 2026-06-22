//! Copilot agent hook runner for guided-plan post-phase verification.

use augur_domain::{
    CopilotAgentHookArgs, CopilotAgentHookFuture, CopilotAgentHookRunner, FailureReason,
    GuidedPlanEvent, HookOutcome, OutputText, ReworkReason,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Maximum duration allowed for a single Copilot agent hook session.
pub const AGENT_HOOK_TIMEOUT: Duration = Duration::from_secs(300);

fn test_hook_outcome(args: &CopilotAgentHookArgs) -> Option<HookOutcome> {
    if args.params.agent == "guided-plan-test-request-rework" {
        Some(HookOutcome::NeedsRework(ReworkReason::from(
            args.params.prompt.to_string(),
        )))
    } else if args.params.agent == "guided-plan-test-approve" {
        Some(HookOutcome::Passed)
    } else {
        None
    }
}

/// Build a copilot hook runner that can be injected into `augur-core`.
pub fn build_copilot_hook_runner() -> CopilotAgentHookRunner {
    std::sync::Arc::new(|args| -> CopilotAgentHookFuture { Box::pin(run_copilot_agent_hook(args)) })
}

#[tracing::instrument(skip(args), level = "info")]
/// Execute a copilot-agent guided-plan hook and return the normalized outcome.
///
/// Test-only override agents (`guided-plan-test-*`) short-circuit deterministically.
/// All other agents run via a bounded Copilot SDK session with timeout handling.
pub async fn run_copilot_agent_hook(args: CopilotAgentHookArgs) -> HookOutcome {
    if let Some(outcome) = test_hook_outcome(&args) {
        return outcome;
    }

    let timeout_result = tokio::time::timeout(AGENT_HOOK_TIMEOUT, run_agent_session(args)).await;
    match timeout_result {
        Ok(outcome) => outcome,
        Err(_) => HookOutcome::Failed(FailureReason::from("agent hook timed out")),
    }
}

async fn run_agent_session(args: CopilotAgentHookArgs) -> HookOutcome {
    let client = match build_hook_client() {
        Ok(c) => c,
        Err(msg) => return HookOutcome::Failed(FailureReason::from(msg)),
    };
    if let Err(e) = client.start().await {
        return HookOutcome::Failed(FailureReason::from(format!(
            "failed to start Copilot client: {e}"
        )));
    }
    let outcome = run_with_client(&client, args).await;
    client.stop().await;
    outcome
}

fn build_hook_client() -> Result<copilot_sdk::Client, String> {
    use copilot_sdk::ClientOptions;
    let cli_path = copilot_sdk::find_copilot_cli()
        .ok_or_else(|| "Copilot CLI not found in PATH".to_string())?;
    let cwd = std::env::current_dir().ok();
    copilot_sdk::Client::new(ClientOptions {
        cli_path: Some(cli_path),
        allow_all_tools: true,
        cli_args: Some(vec!["--allow-all".to_string()]),
        cwd,
        ..Default::default()
    })
    .map_err(|e| format!("failed to create Copilot client: {e}"))
}

async fn run_with_client(client: &copilot_sdk::Client, args: CopilotAgentHookArgs) -> HookOutcome {
    use crate::shared::copilot_permissions::allow_all_handler;
    use copilot_sdk::SessionConfig;

    let working_directory = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    let config = SessionConfig {
        agent: Some(args.params.agent.to_string()),
        tools: vec![approve_phase_tool_def(), request_rework_tool_def()],
        streaming: true,
        config_dir: crate::shared::copilot_session_identity::isolated_config_dir(),
        working_directory,
        client_name: Some(
            crate::shared::copilot_session_identity::DCMK_COPILOT_CLIENT_NAME.to_string(),
        ),
        request_permission: Some(true),
        permission_handler: copilot_sdk::PermissionHandlerField::some(allow_all_handler()),
        ..Default::default()
    };
    let session = match client.create_session(config).await {
        Ok(s) => s,
        Err(e) => {
            return HookOutcome::Failed(FailureReason::from(format!(
                "failed to create session: {e}"
            )))
        }
    };

    let verdict: Arc<Mutex<Option<HookOutcome>>> = Arc::new(Mutex::new(None));
    register_approve_handler(&session, Arc::clone(&verdict)).await;
    register_rework_handler(&session, Arc::clone(&verdict)).await;

    let mut sub = session.subscribe();
    let send_result = session.send(args.params.prompt.to_string()).await;
    let outcome = match send_result {
        Err(e) => HookOutcome::Failed(FailureReason::from(format!("failed to send prompt: {e}"))),
        Ok(_) => stream_events(&mut sub, &args, &verdict).await,
    };
    let _ = session.destroy().await;
    outcome
}

fn approve_phase_tool_def() -> copilot_sdk::Tool {
    copilot_sdk::Tool::new("approve_phase")
        .description("Signal that the current phase is complete and approved.")
        .schema(serde_json::json!({ "type": "object", "properties": {}, "required": [] }))
        .skip_permission(true)
}

fn request_rework_tool_def() -> copilot_sdk::Tool {
    copilot_sdk::Tool::new("request_rework")
        .description("Signal that the current phase needs rework before it can be approved.")
        .schema(serde_json::json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Description of what must be fixed before the phase can be approved."
                }
            },
            "required": ["reason"]
        }))
        .skip_permission(true)
}

async fn register_approve_handler(
    session: &copilot_sdk::Session,
    verdict: Arc<Mutex<Option<HookOutcome>>>,
) {
    use copilot_sdk::ToolResultObject;
    let handler: copilot_sdk::ToolHandler = Arc::new(move |_name, _args: &serde_json::Value| {
        if let Ok(mut guard) = verdict.lock() {
            *guard = Some(HookOutcome::Passed);
        }
        ToolResultObject::text("approved")
    });
    session
        .register_tool_with_handler(approve_phase_tool_def(), Some(handler))
        .await;
}

async fn register_rework_handler(
    session: &copilot_sdk::Session,
    verdict: Arc<Mutex<Option<HookOutcome>>>,
) {
    use copilot_sdk::ToolResultObject;
    let handler: copilot_sdk::ToolHandler = Arc::new(move |_name, args: &serde_json::Value| {
        let reason = args["reason"]
            .as_str()
            .unwrap_or("no reason provided")
            .to_string();
        if let Ok(mut guard) = verdict.lock() {
            *guard = Some(HookOutcome::NeedsRework(ReworkReason::from(reason)));
        }
        ToolResultObject::text("rework requested")
    });
    session
        .register_tool_with_handler(request_rework_tool_def(), Some(handler))
        .await;
}

async fn stream_events(
    sub: &mut copilot_sdk::EventSubscription,
    args: &CopilotAgentHookArgs,
    verdict: &Arc<Mutex<Option<HookOutcome>>>,
) -> HookOutcome {
    let mut stream = ReviewTokenStream::new(args);
    while let Ok(event) = sub.recv().await {
        if should_resolve_verdict(&event.data, &mut stream) {
            return resolve_verdict(&args.params.verdict, verdict, stream.text_buffer());
        }
    }
    HookOutcome::Failed(FailureReason::from("session channel closed"))
}

struct ReviewTokenStream<'a> {
    event_tx: &'a tokio::sync::broadcast::Sender<GuidedPlanEvent>,
    collect_verdict_suffix: bool,
    text_buf: String,
}

impl<'a> ReviewTokenStream<'a> {
    fn new(args: &'a CopilotAgentHookArgs) -> Self {
        Self {
            event_tx: &args.event_tx,
            collect_verdict_suffix: matches!(
                args.params.verdict,
                augur_domain::guided_plan::VerdictKind::VerdictSuffix
            ),
            text_buf: String::new(),
        }
    }

    fn push_token(&mut self, token: &str) {
        if token.is_empty() {
            return;
        }
        let _ = self
            .event_tx
            .send(GuidedPlanEvent::ReviewToken(OutputText::from(token)));
        if self.collect_verdict_suffix {
            self.text_buf.push_str(token);
        }
    }

    fn text_buffer(&self) -> &str {
        &self.text_buf
    }
}

fn should_resolve_verdict(
    event_data: &copilot_sdk::SessionEventData,
    stream: &mut ReviewTokenStream<'_>,
) -> bool {
    use copilot_sdk::SessionEventData;
    match event_data {
        SessionEventData::AssistantMessageDelta(d) => {
            stream.push_token(d.delta_content.as_str());
            false
        }
        SessionEventData::SessionIdle(_) => true,
        _ => false,
    }
}

fn resolve_verdict(
    kind: &augur_domain::guided_plan::VerdictKind,
    verdict: &Arc<Mutex<Option<HookOutcome>>>,
    text_buf: &str,
) -> HookOutcome {
    use augur_domain::guided_plan::VerdictKind;
    match kind {
        VerdictKind::ToolCall => verdict
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .unwrap_or_else(|| HookOutcome::Failed(FailureReason::from("no verdict tool called"))),
        VerdictKind::VerdictSuffix => check_verdict_suffix(text_buf)
            .unwrap_or_else(|| HookOutcome::Failed(FailureReason::from("no verdict suffix found"))),
    }
}

pub fn check_verdict_suffix(text: &str) -> Option<HookOutcome> {
    if text.contains("VERDICT: PASS") {
        Some(HookOutcome::Passed)
    } else if let Some(pos) = text.find("VERDICT: REWORK(") {
        let start = pos + "VERDICT: REWORK(".len();
        text[start..].find(')').map(|offset| {
            HookOutcome::NeedsRework(ReworkReason::from(text[start..start + offset].to_string()))
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::guided_plan::{CopilotAgentHookParams, VerdictKind};

    fn test_args(agent: &str, prompt: &str) -> CopilotAgentHookArgs {
        let (event_tx, _event_rx) = tokio::sync::broadcast::channel(8);
        CopilotAgentHookArgs {
            params: CopilotAgentHookParams {
                agent: agent.into(),
                prompt: prompt.into(),
                verdict: VerdictKind::ToolCall,
            },
            event_tx,
        }
    }

    #[tokio::test]
    async fn test_agent_approve_shortcuts_to_passed() {
        let runner = build_copilot_hook_runner();
        let outcome = runner(test_args("guided-plan-test-approve", "approve")).await;
        assert!(matches!(outcome, HookOutcome::Passed));
    }

    #[tokio::test]
    async fn test_agent_request_rework_shortcuts_to_needs_rework() {
        let runner = build_copilot_hook_runner();
        let outcome = runner(test_args("guided-plan-test-request-rework", "fix the plan")).await;
        assert!(matches!(outcome, HookOutcome::NeedsRework(_)));
        let reason = match outcome {
            HookOutcome::NeedsRework(reason) => reason.to_string(),
            _ => unreachable!(),
        };
        assert!(reason.contains("fix the plan"));
    }
}
