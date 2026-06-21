use crate::domain::newtypes::IsPredicate;
use crate::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use crate::domain::task_types::{
    AgentSpecName, SpawnAgentAck, SpawnAgentChannels, SpawnAgentHandle, SpawnAgentRequest,
    TaskDepth, TaskDispatchState, TaskRunId,
};
use crate::domain::PromptText;
use crate::tools::definition::ToolDefinition;
use crate::tools::handler::{ToolCallResult, ToolHandler};
use tokio::sync::oneshot;

const TOOL_NAME: &str = "task_spawn";

#[derive(bon::Builder)]
pub struct SpawnAgentTool {
    handle: SpawnAgentHandle,
    depth: TaskDepth,
    available_agents: Vec<AgentSpecName>,
}

struct SpawnInvocation {
    request: SpawnAgentRequest,
    agent_name: AgentSpecName,
    run_id: TaskRunId,
    ack_rx: oneshot::Receiver<SpawnAgentAck>,
}

#[async_trait::async_trait]
impl ToolHandler for SpawnAgentTool {
    fn definition(&self) -> ToolDefinition {
        let agent_list = if self.available_agents.is_empty() {
            "no agents found; check .github/agents/".to_string()
        } else {
            self.available_agents
                .iter()
                .map(|a| a.as_ref())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let description = format!(
            "Spawn a named background agent and return a run_id handle immediately. \
             Use task_await/task_status for deterministic fan-in. \
             Available agent names: {agent_list}"
        );
        ToolDefinition::new(
            TOOL_NAME,
            description,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Exact agent name from the available agents list above"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Task prompt to send to the agent"
                    }
                },
                "required": ["name", "prompt"]
            }),
        )
    }

    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let child_depth = match next_child_depth(self.depth) {
            Ok(depth) => depth,
            Err(result) => return result,
        };
        let invocation = match build_invocation(&args, child_depth) {
            Ok(invocation) => invocation,
            Err(result) => return result,
        };
        if let Err(_e) = self.handle.send(invocation.request).await {
            return error_result("spawn agent channel closed");
        }
        match invocation.ack_rx.await {
            Err(_) => error_result("spawn agent dispatch ack oneshot cancelled"),
            Ok(SpawnAgentAck::Completed { status }) => {
                spawn_success_result(&invocation.agent_name, &invocation.run_id, status)
            }
            Ok(ack) => ack_error_result(&invocation.agent_name, &invocation.run_id, ack),
        }
    }
}

fn next_child_depth(depth: TaskDepth) -> Result<TaskDepth, ToolCallResult> {
    depth.increment().ok_or_else(depth_error_result)
}

fn build_invocation(
    args: &serde_json::Value,
    child_depth: TaskDepth,
) -> Result<SpawnInvocation, ToolCallResult> {
    let agent_name = parse_agent_name(args)?;
    let prompt = parse_prompt(args)?;
    let run_id = TaskRunId::new(uuid::Uuid::new_v4().to_string());
    let (ack_tx, ack_rx) = oneshot::channel::<SpawnAgentAck>();
    let (terminal_tx, _terminal_rx) = oneshot::channel::<crate::domain::task_types::TaskSignal>();
    Ok(SpawnInvocation {
        request: SpawnAgentRequest::builder()
            .agent_name(agent_name.clone())
            .prompt(prompt)
            .depth(child_depth)
            .run_id(run_id.clone())
            .channels(
                SpawnAgentChannels::builder()
                    .ack_tx(ack_tx)
                    .terminal_tx(terminal_tx)
                    .build(),
            )
            .build(),
        agent_name,
        run_id,
        ack_rx,
    })
}

fn parse_agent_name(args: &serde_json::Value) -> Result<AgentSpecName, ToolCallResult> {
    match args["name"].as_str() {
        Some(s) if !s.is_empty() => Ok(AgentSpecName::new(s)),
        _ => Err(error_result("missing or empty 'name' argument")),
    }
}

fn parse_prompt(args: &serde_json::Value) -> Result<PromptText, ToolCallResult> {
    match args["prompt"].as_str() {
        Some(s) => Ok(PromptText::new(s)),
        None => Err(error_result("missing 'prompt' argument")),
    }
}

fn depth_error_result() -> ToolCallResult {
    error_result("max nesting depth exceeded")
}

fn ack_error_result(
    agent_name: &AgentSpecName,
    run_id: &TaskRunId,
    ack: SpawnAgentAck,
) -> ToolCallResult {
    let message = match ack {
        SpawnAgentAck::Failed { reason } => format!(
            "task dispatch failed: agent={} run_id={} reason={}",
            agent_name.as_ref(),
            run_id.as_ref(),
            reason.as_str()
        ),
        SpawnAgentAck::Cancelled => format!(
            "task dispatch cancelled: agent={} run_id={}",
            agent_name.as_ref(),
            run_id.as_ref()
        ),
        SpawnAgentAck::Completed { .. } => "task dispatch ack completed unexpectedly".to_string(),
    };
    error_result(&message)
}

fn error_result(message: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(message))
        .is_error(IsPredicate::from(true))
        .build()
}

fn spawn_success_result(
    agent_name: &AgentSpecName,
    run_id: &TaskRunId,
    status: crate::domain::task_types::SpawnDispatchStatus,
) -> ToolCallResult {
    let dispatch = match status.dispatch_state {
        TaskDispatchState::Dispatched => "dispatched".to_string(),
        TaskDispatchState::Queued { position } => format!("queued(position={position})"),
    };
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(format!(
            "[task_spawn agent={} run_id={}] accepted dispatch_state={} max_parallel_workers={} active_runs={} queued_runs={}",
            agent_name.as_ref(),
            run_id.as_ref(),
            dispatch,
            status.queue_snapshot.max_parallel_workers,
            status.queue_snapshot.active_runs,
            status.queue_snapshot.queued_runs
        )))
        .is_error(IsPredicate::from(false))
        .build()
}
