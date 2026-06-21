//! `ExecutorActor` - thin orchestration actor wrapping the CLI session.
//!
//! Spawned by `wiring.rs` when the `copilot-executor` feature is enabled.
//! Without the feature, the actor silently exits so the rest of the system
//! still compiles and tests pass.
//!
//! Startup sequence (feature-enabled):
//! 1. Build `copilot_sdk::Client` using config.
//! 2. Start the client and open a session.
//! 3. Register the `update_plan_step` tool on the session.
//! 4. Subscribe to session events and spawn the event dispatch loop.
//! 5. Enter the command loop, dispatching `ExecutorCmd` to the session.

use super::commands::ExecutorCmd;
use super::commands::SessionEvent;
use super::event_mapper::map_session_event;
use super::executor_ops;
use super::handle::{make_output_channel, ExecutorHandle};
use augur_domain::channels::EXECUTOR_COMMAND_CAPACITY;
use augur_domain::config::types::ExecutorConfig;
use augur_domain::newtypes::{NumericNewtype, TokenCount};
use augur_domain::plan_tree::PlanNodeId;
use augur_domain::string_newtypes::{OutputText, ProcessId, StringNewtype, ToolCallId, ToolName};
use augur_domain::types::AgentOutput;
use tokio::sync::mpsc;

/// Spawn the executor actor and return its handle.
///
/// Creates the command channel, output broadcast channel, and handle, then
/// spawns the actor task. The caller passes the handle to the supervisor via
/// `Box<dyn ExecutorDriver>`.
///
/// When the `copilot-executor` feature is not enabled, the spawned task
/// immediately exits after logging a warning.
#[tracing::instrument(skip_all)]
pub async fn spawn(config: ExecutorConfig) -> (tokio::task::JoinHandle<()>, ExecutorHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(*EXECUTOR_COMMAND_CAPACITY);
    let output_tx = make_output_channel();
    let handle = ExecutorHandle::new(cmd_tx, output_tx.clone());
    let join = tokio::spawn(run(config, cmd_rx, output_tx));
    (join, handle)
}

/// Actor run loop. Exits cleanly on `ExecutorCmd::Stop` or channel close.
async fn run(
    config: ExecutorConfig,
    cmd_rx: mpsc::Receiver<ExecutorCmd>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
) {
    run_with_sdk(config, cmd_rx, output_tx).await;
}

/// Emit a `SessionEvent` onto the output broadcast channel.
///
/// Converts the event via `map_session_event` and sends it if the mapping
/// produces a value. Logs a warning when all subscribers have dropped.
fn emit_event(event: &SessionEvent, output_tx: &tokio::sync::broadcast::Sender<AgentOutput>) {
    if let Some(output) = map_session_event(event)
        && output_tx.send(output).is_err()
    {
        tracing::debug!("ExecutorActor: no output subscribers, event dropped");
    }
}

async fn run_with_sdk(
    config: ExecutorConfig,
    mut cmd_rx: mpsc::Receiver<ExecutorCmd>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
) {
    let Some(client) = start_client(&config, &output_tx).await else {
        return;
    };
    if !check_auth(&client, &output_tx).await {
        let _ = client.stop().await;
        return;
    }
    let Some(session) = create_session(&client, &config, &output_tx).await else {
        let _ = client.stop().await;
        return;
    };
    register_update_plan_step_tool(&session, output_tx.clone()).await;
    spawn_event_dispatch(session.subscribe(), output_tx.clone());
    run_command_loop(&session, &mut cmd_rx).await;
    let _ = client.stop().await;
    tracing::info!("ExecutorActor: stopped cleanly");
}

async fn start_client(
    config: &ExecutorConfig,
    output_tx: &tokio::sync::broadcast::Sender<AgentOutput>,
) -> Option<copilot_sdk::Client> {
    let client_config = executor_ops::build_client_options(config);
    let client = match copilot_sdk::Client::new(client_config) {
        Ok(client) => client,
        Err(error) => {
            emit_sdk_error(
                output_tx,
                &error,
                "ExecutorActor: failed to build SDK client",
            );
            return None;
        }
    };
    if let Err(error) = client.start().await {
        emit_sdk_error(
            output_tx,
            &error,
            "ExecutorActor: failed to start SDK client",
        );
        return None;
    }
    Some(client)
}

async fn check_auth(
    client: &copilot_sdk::Client,
    output_tx: &tokio::sync::broadcast::Sender<AgentOutput>,
) -> bool {
    match client.get_auth_status().await {
        Ok(status) if !status.is_authenticated => {
            tracing::error!("ExecutorActor: not authenticated with GitHub Copilot");
            let _ = output_tx.send(AgentOutput::Error(OutputText::new(
                "GitHub Copilot authentication required. Run `gh auth login` to authenticate.",
            )));
            false
        }
        Err(error) => {
            tracing::warn!(error = %error, "ExecutorActor: auth status check failed; proceeding");
            true
        }
        Ok(_) => true,
    }
}

async fn create_session(
    client: &copilot_sdk::Client,
    config: &ExecutorConfig,
    output_tx: &tokio::sync::broadcast::Sender<AgentOutput>,
) -> Option<std::sync::Arc<copilot_sdk::Session>> {
    let session_config = executor_ops::build_session_config(config);
    match client.create_session(session_config).await {
        Ok(session) => Some(session),
        Err(error) => {
            emit_sdk_error(output_tx, &error, "ExecutorActor: failed to create session");
            None
        }
    }
}

fn emit_sdk_error(
    output_tx: &tokio::sync::broadcast::Sender<AgentOutput>,
    error: &impl std::fmt::Display,
    message: &str,
) {
    tracing::error!(error = %error, "{message}");
    let _ = output_tx.send(AgentOutput::Error(OutputText::new(error.to_string())));
}

pub fn spawn_event_dispatch(
    mut event_rx: copilot_sdk::EventSubscription,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
) {
    tokio::spawn(async move {
        while let Ok(sdk_event) = event_rx.recv().await {
            let local = translate_sdk_event(&sdk_event.data);
            emit_event(&local, &output_tx);
        }
    });
}

pub async fn run_command_loop(
    session: &copilot_sdk::Session,
    cmd_rx: &mut mpsc::Receiver<ExecutorCmd>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        if !handle_executor_cmd(session, cmd).await {
            break;
        }
    }
}

async fn handle_executor_cmd(session: &copilot_sdk::Session, cmd: ExecutorCmd) -> bool {
    match cmd {
        ExecutorCmd::Stop => false,
        ExecutorCmd::ShellExec { command, reply_tx } => {
            handle_shell_exec_cmd(session, command, reply_tx).await;
            true
        }
        cmd => {
            handle_session_control_cmd(session, to_session_control_cmd(cmd)).await;
            true
        }
    }
}

async fn handle_shell_exec_cmd(
    session: &copilot_sdk::Session,
    command: augur_domain::ShellCommand,
    reply_tx: tokio::sync::oneshot::Sender<super::commands::ShellExecResult>,
) {
    let result = run_shell_exec(session, command).await;
    let _ = reply_tx.send(result);
}

enum SessionControlCmd {
    SendPrompt(augur_domain::PromptText),
    SetMode(augur_domain::traits::ExecutorMode),
    Compact,
}

fn to_session_control_cmd(cmd: ExecutorCmd) -> SessionControlCmd {
    match cmd {
        ExecutorCmd::SendPrompt { content } => SessionControlCmd::SendPrompt(content),
        ExecutorCmd::SetMode { mode } => SessionControlCmd::SetMode(mode),
        ExecutorCmd::Compact => SessionControlCmd::Compact,
        ExecutorCmd::Stop | ExecutorCmd::ShellExec { .. } => {
            unreachable!("non-session control command routed as session control")
        }
    }
}

async fn handle_session_control_cmd(session: &copilot_sdk::Session, cmd: SessionControlCmd) {
    match cmd {
        SessionControlCmd::SendPrompt(content) => send_prompt(session, content).await,
        SessionControlCmd::SetMode(mode) => set_session_mode(session, mode).await,
        SessionControlCmd::Compact => compact_session(session).await,
    }
}

async fn send_prompt(session: &copilot_sdk::Session, content: augur_domain::PromptText) {
    if let Err(error) = session.send(content.as_str()).await {
        tracing::error!(error = %error, "ExecutorActor: send_prompt failed");
    }
}

async fn set_session_mode(
    session: &copilot_sdk::Session,
    mode: augur_domain::traits::ExecutorMode,
) {
    if let Err(error) = session.set_mode(to_sdk_mode(mode)).await {
        tracing::error!(error = %error, "ExecutorActor: set_mode failed");
    }
}

async fn compact_session(session: &copilot_sdk::Session) {
    if let Err(error) = session.compact().await {
        tracing::error!(error = %error, "ExecutorActor: compact failed");
    }
}

async fn run_shell_exec(
    session: &copilot_sdk::Session,
    command: augur_domain::ShellCommand,
) -> super::commands::ShellExecResult {
    let opts = copilot_sdk::ShellExecOptions {
        command: command.into_inner(),
        cwd: None,
        env: None,
    };
    match session.shell_exec(opts).await {
        Ok(result) => super::commands::ShellExecResult {
            process_id: ProcessId::from(result.process_id),
        },
        Err(error) => {
            tracing::error!(error = %error, "ExecutorActor: shell_exec failed");
            super::commands::ShellExecResult {
                process_id: ProcessId::from(""),
            }
        }
    }
}

pub async fn register_update_plan_step_tool(
    session: &copilot_sdk::Session,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
) {
    use copilot_sdk::Tool;

    let tool = Tool::new("update_plan_step")
        .description("Report progress on a plan tree node. Call when starting, completing, or failing a step.")
        .parameter("node_id", "string", "The PlanNodeId of the step being updated", true)
        .parameter("status", "string", "One of: in_progress, done, failed", true)
        .parameter("notes", "string", "Failure reason or completion notes", false)
        .skip_permission(true);

    let tx = output_tx.clone();
    let handler: copilot_sdk::ToolHandler =
        std::sync::Arc::new(move |_name, args: &serde_json::Value| {
            let node_id = args["node_id"].as_str().unwrap_or("").to_owned();
            let status = args["status"].as_str().unwrap_or("").to_owned();
            let notes = args["notes"].as_str().map(|s| s.to_owned());
            let event = SessionEvent::PlanNodeUpdated {
                node_id: PlanNodeId::new(node_id),
                status,
                notes,
            };
            emit_event(&event, &tx);
            copilot_sdk::ToolResultObject::text("ok")
        });
    session
        .register_tool_with_handler(tool, Some(handler))
        .await;
}

fn translate_sdk_event(event: &copilot_sdk::SessionEventData) -> SessionEvent {
    translate_assistant_event(event)
        .or_else(|| translate_tool_event(event))
        .or_else(|| translate_session_event(event))
        .unwrap_or(SessionEvent::Unknown)
}

fn translate_assistant_event(event: &copilot_sdk::SessionEventData) -> Option<SessionEvent> {
    translate_assistant_message_event(event)
        .or_else(|| translate_assistant_usage_event(event))
        .or_else(|| translate_assistant_intent_event(event))
}

fn translate_tool_event(event: &copilot_sdk::SessionEventData) -> Option<SessionEvent> {
    translate_tool_lifecycle_event(event)
        .or_else(|| translate_tool_progress_event(event))
        .or_else(|| translate_tool_partial_result_event(event))
}

fn translate_assistant_message_event(
    event: &copilot_sdk::SessionEventData,
) -> Option<SessionEvent> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::AssistantMessageDelta(d) => Some(SessionEvent::AssistantMessageDelta {
            content: OutputText::new(d.delta_content.clone()),
        }),
        E::AssistantMessage(_) => Some(SessionEvent::AssistantMessageComplete),
        _ => None,
    }
}

fn translate_assistant_usage_event(event: &copilot_sdk::SessionEventData) -> Option<SessionEvent> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::AssistantUsage(d) => Some(SessionEvent::AssistantUsage {
            input_tokens: d.input_tokens.map(|v| TokenCount::new(v as u64)),
            output_tokens: d.output_tokens.map(|v| TokenCount::new(v as u64)),
            cache_read_tokens: d.cache_read_tokens.map(|v| TokenCount::new(v as u64)),
        }),
        _ => None,
    }
}

fn translate_assistant_intent_event(event: &copilot_sdk::SessionEventData) -> Option<SessionEvent> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::AssistantIntent(d) => Some(SessionEvent::AssistantIntent {
            intent: OutputText::new(d.intent.clone()),
        }),
        _ => None,
    }
}

fn translate_tool_lifecycle_event(event: &copilot_sdk::SessionEventData) -> Option<SessionEvent> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::ToolExecutionStart(d) => Some(SessionEvent::ToolExecutionStart {
            tool_name: ToolName::new(d.tool_name.clone()),
            args: d.arguments.clone().unwrap_or(serde_json::Value::Null),
        }),
        E::ToolExecutionComplete(d) => Some(SessionEvent::ToolExecutionComplete {
            tool_call_id: ToolCallId::new(d.tool_call_id.clone()),
        }),
        _ => None,
    }
}

fn translate_tool_progress_event(event: &copilot_sdk::SessionEventData) -> Option<SessionEvent> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::ToolExecutionProgress(d) => Some(SessionEvent::ToolProgress {
            tool_call_id: ToolCallId::new(d.tool_call_id.clone()),
            message: OutputText::new(d.progress_message.clone()),
        }),
        _ => None,
    }
}

fn translate_tool_partial_result_event(
    event: &copilot_sdk::SessionEventData,
) -> Option<SessionEvent> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::ToolExecutionPartialResult(d) => Some(SessionEvent::ToolPartialResult {
            tool_call_id: ToolCallId::new(d.tool_call_id.clone()),
            output: OutputText::new(d.partial_output.clone()),
        }),
        _ => None,
    }
}

fn translate_session_event(event: &copilot_sdk::SessionEventData) -> Option<SessionEvent> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::SessionError(d) => Some(SessionEvent::SessionError {
            message: d.message.clone(),
        }),
        E::SessionIdle(_) => Some(SessionEvent::SessionIdle),
        _ => None,
    }
}

fn to_sdk_mode(mode: augur_domain::traits::ExecutorMode) -> copilot_sdk::SessionMode {
    use augur_domain::traits::ExecutorMode as M;
    use copilot_sdk::SessionMode as S;
    match mode {
        M::Interactive => S::Interactive,
        M::Plan => S::Plan,
        M::Autopilot => S::Autopilot,
    }
}
