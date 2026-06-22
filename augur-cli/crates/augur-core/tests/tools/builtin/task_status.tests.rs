use augur_core::tools::builtin::task_status::TaskStatusTool;
use augur_core::tools::handler::ToolHandler;
use augur_domain::domain::string_newtypes::StringNewtype;
use augur_domain::domain::task_types::{
    TaskOrchestratorPort, TaskRunLifecycleState, TaskRunStatusEntry, TaskRunStatusSnapshot,
    TaskSignal,
};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

struct MockStatusOrchestrator {
    snapshot: Mutex<Option<TaskRunStatusSnapshot>>,
    queried: Mutex<bool>,
}

impl MockStatusOrchestrator {
    fn new(snapshot: TaskRunStatusSnapshot) -> Self {
        Self {
            snapshot: Mutex::new(Some(snapshot)),
            queried: Mutex::new(false),
        }
    }
}

impl TaskOrchestratorPort for MockStatusOrchestrator {
    fn await_run(
        &self,
        _run_id: augur_domain::domain::task_types::TaskRunId,
    ) -> anyhow::Result<oneshot::Receiver<augur_domain::domain::task_types::AwaitRunResult>> {
        anyhow::bail!("await_run not used in task_status tests")
    }

    fn await_any(
        &self,
        _run_ids: Vec<augur_domain::domain::task_types::TaskRunId>,
    ) -> anyhow::Result<oneshot::Receiver<augur_domain::domain::task_types::AwaitRunResult>> {
        anyhow::bail!("await_any not used in task_status tests")
    }

    fn query_status(&self) -> anyhow::Result<oneshot::Receiver<TaskRunStatusSnapshot>> {
        *self.queried.lock().expect("lock queried") = true;
        let (tx, rx) = oneshot::channel();
        let snapshot = self
            .snapshot
            .lock()
            .expect("lock snapshot")
            .take()
            .expect("snapshot payload");
        let _ = tx.send(snapshot);
        Ok(rx)
    }
}

#[tokio::test]
async fn status_tool_requests_snapshot_and_formats_entries() {
    let backing = Arc::new(MockStatusOrchestrator::new(
        TaskRunStatusSnapshot::builder()
            .max_parallel_workers(4)
            .active_runs(1)
            .queued_runs(2)
            .terminal_ready_runs(1)
            .runs(vec![
                TaskRunStatusEntry::builder()
                    .run_id(augur_domain::domain::task_types::TaskRunId::new("run-a"))
                    .state(TaskRunLifecycleState::Active)
                    .build(),
                TaskRunStatusEntry::builder()
                    .run_id(augur_domain::domain::task_types::TaskRunId::new("run-b"))
                    .state(TaskRunLifecycleState::TerminalReady {
                        signal: TaskSignal::Failed {
                            reason: augur_domain::domain::OutputText::new("boom"),
                        },
                    })
                    .build(),
            ])
            .build(),
    ));
    let tool = TaskStatusTool::builder()
        .orchestrator(backing.clone() as Arc<dyn TaskOrchestratorPort>)
        .build();
    let result = tool.execute(serde_json::json!({})).await;
    assert!(!result.is_error, "status should succeed");
    assert!(
        result.output.as_str().contains("run_id=run-a state=active"),
        "output: {}",
        result.output.as_str()
    );
    assert!(
        result
            .output
            .as_str()
            .contains("run_id=run-b state=terminal_ready(failed:boom)"),
        "output: {}",
        result.output.as_str()
    );
    assert!(*backing.queried.lock().expect("lock queried"));
}
