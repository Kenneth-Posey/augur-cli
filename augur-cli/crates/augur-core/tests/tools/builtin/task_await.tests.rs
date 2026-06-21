use augur_core::tools::builtin::task_await::TaskAwaitTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use augur_domain::domain::task_types::{
    AwaitRunResult, TaskOrchestratorPort, TaskRunId, TaskSignal,
};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

struct MockAwaitOrchestrator {
    await_run_response: Mutex<Option<AwaitRunResult>>,
    await_any_response: Mutex<Option<AwaitRunResult>>,
    last_run_id: Mutex<Option<TaskRunId>>,
    last_run_ids: Mutex<Option<Vec<TaskRunId>>>,
}

impl MockAwaitOrchestrator {
    fn new(await_run_response: AwaitRunResult, await_any_response: AwaitRunResult) -> Self {
        Self {
            await_run_response: Mutex::new(Some(await_run_response)),
            await_any_response: Mutex::new(Some(await_any_response)),
            last_run_id: Mutex::new(None),
            last_run_ids: Mutex::new(None),
        }
    }
}

impl TaskOrchestratorPort for MockAwaitOrchestrator {
    fn await_run(&self, run_id: TaskRunId) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>> {
        *self.last_run_id.lock().expect("lock last_run_id") = Some(run_id);
        let (tx, rx) = oneshot::channel();
        let payload = self
            .await_run_response
            .lock()
            .expect("lock await_run_response")
            .take()
            .expect("await_run payload");
        let _ = tx.send(payload);
        Ok(rx)
    }

    fn await_any(
        &self,
        run_ids: Vec<TaskRunId>,
    ) -> anyhow::Result<oneshot::Receiver<AwaitRunResult>> {
        *self.last_run_ids.lock().expect("lock last_run_ids") = Some(run_ids);
        let (tx, rx) = oneshot::channel();
        let payload = self
            .await_any_response
            .lock()
            .expect("lock await_any_response")
            .take()
            .expect("await_any payload");
        let _ = tx.send(payload);
        Ok(rx)
    }

    fn query_status(
        &self,
    ) -> anyhow::Result<oneshot::Receiver<augur_domain::domain::task_types::TaskRunStatusSnapshot>>
    {
        anyhow::bail!("query_status not used in task_await tests")
    }
}

#[tokio::test]
async fn await_single_run_consumes_terminal_payload() {
    let orchestrator = Arc::new(MockAwaitOrchestrator::new(
        AwaitRunResult::ConsumedTerminal {
            run_id: TaskRunId::new("run-1"),
            signal: TaskSignal::Completed {
                output: augur_domain::domain::AccumulatedText::new("done"),
            },
        },
        AwaitRunResult::AlreadyConsumed {
            run_id: TaskRunId::new("unused"),
        },
    )) as Arc<dyn TaskOrchestratorPort>;
    let tool = TaskAwaitTool::builder().orchestrator(orchestrator).build();
    let result = tool.execute(serde_json::json!({"run_id": "run-1"})).await;
    assert!(!result.is_error, "completed run should be success");
    assert!(
        result.output.as_str().contains("done"),
        "expected terminal payload: {}",
        result.output.as_str()
    );
}

#[tokio::test]
async fn await_any_mode_enqueues_await_any_command() {
    let backing = Arc::new(MockAwaitOrchestrator::new(
        AwaitRunResult::AlreadyConsumed {
            run_id: TaskRunId::new("unused"),
        },
        AwaitRunResult::AlreadyConsumed {
            run_id: TaskRunId::new("run-b"),
        },
    ));
    let tool = TaskAwaitTool::builder()
        .orchestrator(backing.clone() as Arc<dyn TaskOrchestratorPort>)
        .build();
    let result = tool
        .execute(serde_json::json!({"mode":"any","run_ids":["run-a","run-b"]}))
        .await;
    assert!(
        !result.is_error,
        "already-consumed await response should be deterministic success"
    );
    assert!(
        result.output.as_str().contains("already consumed"),
        "output: {}",
        result.output.as_str()
    );
    let captured = backing
        .last_run_ids
        .lock()
        .expect("lock last_run_ids")
        .clone()
        .expect("captured run ids");
    assert_eq!(captured.len(), 2);
}

#[tokio::test]
async fn await_unknown_run_returns_error() {
    let orchestrator = Arc::new(MockAwaitOrchestrator::new(
        AwaitRunResult::UnknownRun {
            run_id: TaskRunId::new("missing"),
        },
        AwaitRunResult::AlreadyConsumed {
            run_id: TaskRunId::new("unused"),
        },
    )) as Arc<dyn TaskOrchestratorPort>;
    let tool = TaskAwaitTool::builder().orchestrator(orchestrator).build();
    let result = tool.execute(serde_json::json!({"run_id":"missing"})).await;
    assert!(result.is_error, "unknown run must be error");
}
