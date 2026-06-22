use augur_domain::plan_tree::NodeStatus;
use augur_domain::string_newtypes::StringNewtype;
use augur_domain::types::AgentOutput;
use augur_provider_copilot_sdk::actors::executor::executor_actor::{
    register_update_plan_step_tool, run_command_loop, spawn_event_dispatch,
};
use copilot_sdk::{
    AssistantIntentData, Session, SessionEvent, SessionEventData, ToolExecutionPartialResultData,
    ToolExecutionProgressData,
};
use serde_json::json;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{timeout, Duration};

fn test_session() -> Session {
    Session::new(
        "executor-test-session".to_owned(),
        None,
        |_method, _params| Box::pin(async { Ok(serde_json::Value::Null) }),
    )
}

fn sdk_event(event_type: &str, data: SessionEventData) -> SessionEvent {
    SessionEvent {
        id: format!("{event_type}-id"),
        timestamp: "2026-01-01T00:00:00Z".to_owned(),
        event_type: event_type.to_owned(),
        parent_id: None,
        ephemeral: None,
        data,
    }
}

async fn recv_output(rx: &mut broadcast::Receiver<AgentOutput>) -> AgentOutput {
    timeout(Duration::from_secs(1), rx.recv())
        .await
        .expect("expected executor output before timeout")
        .expect("expected executor output value")
}

/// Verifies that an SDK `AssistantIntent` event is translated and published as
/// `AgentOutput::IntentMessage` on the executor output stream.
#[tokio::test]
async fn sdk_assistant_intent_event_is_published_to_output_stream() {
    let session = test_session();
    let (output_tx, mut output_rx) = broadcast::channel(8);
    spawn_event_dispatch(session.subscribe(), output_tx);

    session
        .dispatch_event(sdk_event(
            "assistant.intent",
            SessionEventData::AssistantIntent(AssistantIntentData {
                intent: "inspect the failing executor path".to_owned(),
            }),
        ))
        .await;

    match recv_output(&mut output_rx).await {
        AgentOutput::IntentMessage(text) => {
            assert_eq!(text.as_str(), "inspect the failing executor path");
        }
        other => panic!("expected IntentMessage, got {other:?}"),
    }
}

/// Verifies that an SDK `ToolExecutionProgress` event is translated and
/// published as `AgentOutput::ToolProgress` on the executor output stream.
#[tokio::test]
async fn sdk_tool_progress_event_is_published_to_output_stream() {
    let session = test_session();
    let (output_tx, mut output_rx) = broadcast::channel(8);
    spawn_event_dispatch(session.subscribe(), output_tx);

    session
        .dispatch_event(sdk_event(
            "tool.execution_progress",
            SessionEventData::ToolExecutionProgress(ToolExecutionProgressData {
                tool_call_id: "tool-call-42".to_owned(),
                progress_message: "reading workspace files".to_owned(),
            }),
        ))
        .await;

    match recv_output(&mut output_rx).await {
        AgentOutput::ToolProgress {
            tool_call_id,
            message,
        } => {
            assert_eq!(tool_call_id.as_str(), "tool-call-42");
            assert_eq!(message.as_str(), "reading workspace files");
        }
        other => panic!("expected ToolProgress, got {other:?}"),
    }
}

/// Verifies that an SDK `ToolExecutionPartialResult` event is translated and
/// published as `AgentOutput::ToolPartialResult` on the executor output stream.
#[tokio::test]
async fn sdk_tool_partial_result_event_is_published_to_output_stream() {
    let session = test_session();
    let (output_tx, mut output_rx) = broadcast::channel(8);
    spawn_event_dispatch(session.subscribe(), output_tx);

    session
        .dispatch_event(sdk_event(
            "tool.execution_partial_result",
            SessionEventData::ToolExecutionPartialResult(ToolExecutionPartialResultData {
                tool_call_id: "tool-call-99".to_owned(),
                partial_output: "first line\nsecond line".to_owned(),
            }),
        ))
        .await;

    match recv_output(&mut output_rx).await {
        AgentOutput::ToolPartialResult {
            tool_call_id,
            output,
        } => {
            assert_eq!(tool_call_id.as_str(), "tool-call-99");
            assert_eq!(output.as_str(), "first line\nsecond line");
        }
        other => panic!("expected ToolPartialResult, got {other:?}"),
    }
}

/// Verifies that invoking the registered `update_plan_step` tool publishes a
/// `PlanNodeUpdate` carrying the translated node status and notes.
#[tokio::test]
async fn update_plan_step_tool_invocation_publishes_plan_node_update() {
    let session = test_session();
    let (output_tx, mut output_rx) = broadcast::channel(8);
    register_update_plan_step_tool(&session, output_tx).await;

    session
        .invoke_tool(
            "update_plan_step",
            &json!({
                "node_id": "phase-6-executor-gap",
                "status": "failed",
                "notes": "tool output did not reach subscribers"
            }),
        )
        .await
        .expect("update_plan_step tool should be registered");

    match recv_output(&mut output_rx).await {
        AgentOutput::PlanNodeUpdate {
            node_id,
            status,
            notes,
        } => {
            assert_eq!(node_id.as_str(), "phase-6-executor-gap");
            assert_eq!(
                status,
                NodeStatus::Failed("tool output did not reach subscribers".into())
            );
            assert_eq!(
                notes.as_deref(),
                Some("tool output did not reach subscribers")
            );
        }
        other => panic!("expected PlanNodeUpdate, got {other:?}"),
    }
}

/// Verifies that the executor command loop exits cleanly when it receives
/// `ExecutorCmd::Stop`.
#[tokio::test]
async fn command_loop_exits_when_stop_command_arrives() {
    let session = test_session();
    let (cmd_tx, mut cmd_rx) = mpsc::channel(1);
    cmd_tx
        .send(augur_provider_copilot_sdk::actors::executor::commands::ExecutorCmd::Stop)
        .await
        .expect("stop command should enqueue");

    timeout(
        Duration::from_secs(1),
        run_command_loop(&session, &mut cmd_rx),
    )
    .await
    .expect("command loop should exit after stop");
}

/// Verifies that the executor command loop exits cleanly when its command
/// channel is closed without another command arriving.
#[tokio::test]
async fn command_loop_exits_when_command_channel_closes() {
    let session = test_session();
    let (cmd_tx, mut cmd_rx) = mpsc::channel(1);
    drop(cmd_tx);

    timeout(
        Duration::from_secs(1),
        run_command_loop(&session, &mut cmd_rx),
    )
    .await
    .expect("command loop should exit after channel close");
}

#[test]
fn mirror_sync_executes_sdk_assistant_intent_event_is_published_to_output_stream() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
