//! Built-in task_await tool: deterministic fan-in by run_id.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::domain::task_types::{
    AwaitRunResult, TaskOrchestratorPort, TaskRunId, TaskSignal,
};
use augur_domain::tools::definition::ToolDefinition;
use std::sync::Arc;

const TOOL_NAME: &str = "task_await";

#[derive(bon::Builder, Clone)]
/// Tool that blocks on a correlated background run and consumes terminal output.
pub struct TaskAwaitTool {
    orchestrator: Arc<dyn TaskOrchestratorPort>,
}

#[async_trait::async_trait]
impl ToolHandler for TaskAwaitTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Await one run_id or any-of run_ids and consume terminal payload deterministically.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "run_id": { "type": "string", "description": "Single run id to await" },
                    "run_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Candidate run ids for any-of await"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["single", "any"],
                        "description": "Await mode; default is single"
                    }
                }
            }),
        )
    }

    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let mode = args["mode"].as_str().unwrap_or("single");
        let run_id = args["run_id"]
            .as_str()
            .map(TaskRunId::new)
            .filter(|id| !id.as_ref().is_empty());
        let run_ids = args["run_ids"]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str())
                    .map(TaskRunId::new)
                    .filter(|id| !id.as_ref().is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let receiver = if mode == "any" {
            if run_ids.is_empty() {
                return error_result("task_await any mode requires non-empty run_ids");
            }
            match self.orchestrator.await_any(run_ids) {
                Ok(receiver) => receiver,
                Err(error) => return error_result(&format!("task_await enqueue failed: {error}")),
            }
        } else {
            let Some(run_id) = run_id.or_else(|| run_ids.into_iter().next()) else {
                return error_result("task_await requires run_id");
            };
            match self.orchestrator.await_run(run_id) {
                Ok(receiver) => receiver,
                Err(error) => return error_result(&format!("task_await enqueue failed: {error}")),
            }
        };
        match receiver.await {
            Ok(result) => await_result_to_tool_call_result(result),
            Err(_) => error_result("task_await response channel cancelled"),
        }
    }
}

fn await_result_to_tool_call_result(result: AwaitRunResult) -> ToolCallResult {
    match result {
        AwaitRunResult::ConsumedTerminal { run_id, signal } => signal_result(run_id, signal),
        AwaitRunResult::AlreadyConsumed { run_id } => ToolCallResult::builder()
            .name(ToolName::new(TOOL_NAME))
            .output(OutputText::new(format!(
                "[task_await run_id={}] terminal already consumed",
                run_id.as_ref()
            )))
            .is_error(IsPredicate::from(false))
            .build(),
        AwaitRunResult::UnknownRun { run_id } => {
            error_result(&format!("task_await unknown run_id={}", run_id.as_ref()))
        }
    }
}

fn signal_result(run_id: TaskRunId, signal: TaskSignal) -> ToolCallResult {
    match signal {
        TaskSignal::Completed { output } => ToolCallResult::builder()
            .name(ToolName::new(TOOL_NAME))
            .output(OutputText::new(format!(
                "[task_await run_id={}] completed\n{}",
                run_id.as_ref(),
                output.as_str()
            )))
            .is_error(IsPredicate::from(false))
            .build(),
        TaskSignal::Failed { reason } => error_result(&format!(
            "task_await run_id={} failed reason={}",
            run_id.as_ref(),
            reason.as_str()
        )),
        TaskSignal::Cancelled => {
            error_result(&format!("task_await run_id={} cancelled", run_id.as_ref()))
        }
    }
}

fn error_result(message: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(message))
        .is_error(IsPredicate::from(true))
        .build()
}
