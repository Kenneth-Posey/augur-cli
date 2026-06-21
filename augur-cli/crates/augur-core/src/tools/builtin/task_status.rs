//! Built-in task_status tool: list queued/active/terminal run state.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::domain::task_types::{
    TaskOrchestratorPort, TaskRunLifecycleState, TaskRunStatusSnapshot, TaskSignal,
};
use augur_domain::tools::definition::ToolDefinition;
use std::sync::Arc;

const TOOL_NAME: &str = "task_status";

#[derive(bon::Builder, Clone)]
/// Tool that returns queued/active/terminal run lifecycle state snapshots.
pub struct TaskStatusTool {
    orchestrator: Arc<dyn TaskOrchestratorPort>,
}

#[async_trait::async_trait]
impl ToolHandler for TaskStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "List task orchestration status: queued, active, terminal-ready, and consumed runs.",
            serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        )
    }

    async fn execute(&self, _args: serde_json::Value) -> ToolCallResult {
        let receiver = match self.orchestrator.query_status() {
            Ok(receiver) => receiver,
            Err(error) => return error_result(&format!("task_status enqueue failed: {error}")),
        };
        match receiver.await {
            Ok(snapshot) => status_result(snapshot),
            Err(_) => error_result("task_status response channel cancelled"),
        }
    }
}

fn status_result(snapshot: TaskRunStatusSnapshot) -> ToolCallResult {
    let mut lines = vec![format!(
        "[task_status] max_parallel_workers={} active_runs={} queued_runs={} terminal_ready_runs={}",
        snapshot.max_parallel_workers,
        snapshot.active_runs,
        snapshot.queued_runs,
        snapshot.terminal_ready_runs
    )];
    for run in snapshot.runs {
        lines.push(format!(
            "run_id={} state={}",
            run.run_id.as_ref(),
            lifecycle_label(run.state)
        ));
    }
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(lines.join("\n")))
        .is_error(IsPredicate::from(false))
        .build()
}

fn lifecycle_label(state: TaskRunLifecycleState) -> String {
    match state {
        TaskRunLifecycleState::Pending => "pending".to_string(),
        TaskRunLifecycleState::Active => "active".to_string(),
        TaskRunLifecycleState::TerminalReady { signal } => match signal {
            TaskSignal::Completed { .. } => "terminal_ready(completed)".to_string(),
            TaskSignal::Failed { reason } => format!("terminal_ready(failed:{})", reason.as_str()),
            TaskSignal::Cancelled => "terminal_ready(cancelled)".to_string(),
        },
        TaskRunLifecycleState::TerminalConsumed => "terminal_consumed".to_string(),
    }
}

fn error_result(message: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(message))
        .is_error(IsPredicate::from(true))
        .build()
}
