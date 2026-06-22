use augur_core::tools::builtin::spawn_agent::SpawnAgentTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::domain::task_types::{
    AgentSpecName, SpawnAgentAck, SpawnAgentHandle, SpawnAgentRequest, SpawnDispatchStatus,
    TaskDepth, TaskDispatchState, TaskQueueSnapshot, MAX_TASK_DEPTH,
};
use tokio::sync::mpsc;

#[tokio::test]
async fn depth_at_max_returns_error() {
    let (tx, _rx) = mpsc::channel::<SpawnAgentRequest>(1);
    let tool = SpawnAgentTool::builder()
        .handle(SpawnAgentHandle(tx))
        .depth(TaskDepth(MAX_TASK_DEPTH))
        .available_agents(vec![])
        .build();
    let result = tool
        .execute(serde_json::json!({"name":"agent","prompt":"do"}))
        .await;
    assert!(result.is_error, "depth cap should return error");
}

#[tokio::test]
async fn spawn_returns_run_id_without_waiting_for_terminal_signal() {
    let (tx, mut rx) = mpsc::channel::<SpawnAgentRequest>(1);
    let tool = SpawnAgentTool::builder()
        .handle(SpawnAgentHandle(tx))
        .depth(TaskDepth::root())
        .available_agents(vec![AgentSpecName::new("code-reviewer")])
        .build();
    let task = tokio::spawn(async move {
        tool.execute(serde_json::json!({"name":"code-reviewer","prompt":"inspect"}))
            .await
    });
    let request = rx.recv().await.expect("spawn request");
    let run_id = request.run_id.clone();
    let _ = request.channels.ack_tx.send(SpawnAgentAck::Completed {
        status: SpawnDispatchStatus::builder()
            .run_id(run_id.clone())
            .dispatch_state(TaskDispatchState::Dispatched)
            .queue_snapshot(
                TaskQueueSnapshot::builder()
                    .max_parallel_workers(4)
                    .active_runs(1)
                    .queued_runs(0)
                    .build(),
            )
            .build(),
    });
    let result = task.await.expect("task join");
    assert!(!result.is_error, "spawn ack should succeed");
    assert!(
        result.output.as_str().contains(run_id.as_ref()),
        "spawn result must contain run_id: {}",
        result.output.as_str()
    );
    assert!(
        result.output.as_str().contains("dispatch_state=dispatched"),
        "spawn result should expose backpressure metadata: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn queued_ack_exposes_backpressure_metadata() {
    let (tx, mut rx) = mpsc::channel::<SpawnAgentRequest>(1);
    let tool = SpawnAgentTool::builder()
        .handle(SpawnAgentHandle(tx))
        .depth(TaskDepth::root())
        .available_agents(vec![])
        .build();
    let task = tokio::spawn(async move {
        tool.execute(serde_json::json!({"name":"agent","prompt":"task"}))
            .await
    });
    let request = rx.recv().await.expect("spawn request");
    let _ = request.channels.ack_tx.send(SpawnAgentAck::Completed {
        status: SpawnDispatchStatus::builder()
            .run_id(request.run_id.clone())
            .dispatch_state(TaskDispatchState::Queued { position: 3 })
            .queue_snapshot(
                TaskQueueSnapshot::builder()
                    .max_parallel_workers(2)
                    .active_runs(2)
                    .queued_runs(4)
                    .build(),
            )
            .build(),
    });
    let result = task.await.expect("task join");
    assert!(
        result.output.as_str().contains("queued(position=3)"),
        "queued metadata should be returned: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn failed_ack_returns_error() {
    let (tx, mut rx) = mpsc::channel::<SpawnAgentRequest>(1);
    let tool = SpawnAgentTool::builder()
        .handle(SpawnAgentHandle(tx))
        .depth(TaskDepth::root())
        .available_agents(vec![])
        .build();
    let task = tokio::spawn(async move {
        tool.execute(serde_json::json!({"name":"agent","prompt":"task"}))
            .await
    });
    let request = rx.recv().await.expect("spawn request");
    let _ = request.channels.ack_tx.send(SpawnAgentAck::Failed {
        reason: OutputText::new("queue full"),
    });
    let result = task.await.expect("task join");
    assert!(result.is_error, "failed ack must map to error");
}

#[test]
fn definition_uses_split_spawn_name() {
    let (tx, _rx) = mpsc::channel::<SpawnAgentRequest>(1);
    let tool = SpawnAgentTool::builder()
        .handle(SpawnAgentHandle(tx))
        .depth(TaskDepth::root())
        .available_agents(vec![])
        .build();
    assert_eq!(tool.definition().name.as_str(), "task_spawn");
}
