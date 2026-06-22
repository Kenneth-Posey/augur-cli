//! Per-turn logging and persistence helpers for `CopilotChatActor`.
//!
//! Extracted from `actor.rs` to keep the actor file within the 200-line logic
//! threshold. Covers in-flight token accumulation, turn completion recording,
//! and incremental log draining between commands.

use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::string_newtypes::OutputText;
use augur_domain::types::AgentOutput;
use augur_domain::{HistoryAdapterHandle, LoggerHandle};

const COPILOT_ENDPOINT: &str = "copilot";

/// Grouped handle fields for [`LogState`].
///
/// Bundles logger, history adapter, and persistence so [`LogState`]
/// stays within the 5-field limit.
/// Consumers: `LogState`, `build_log_state`.
#[derive(bon::Builder)]
pub struct LogHandles {
    /// Logger handle for per-turn JSONL message logging.
    pub logger: LoggerHandle,
    /// History adapter handle for fire-and-forget conversation message recording.
    pub history_adapter: HistoryAdapterHandle,
    /// Persistence handle for saving completed turns to disk.
    pub persistence: PersistenceHandle,
}

#[derive(bon::Builder)]
/// Accumulated per-turn logging and persistence state for the copilot command loop.
///
/// Tracks the user message sent at the start of a turn and the assistant tokens
/// received so far. When `TurnComplete` is observed, both are logged and persisted
/// together so the history file reflects only completed turns.
///
/// `pending_user` holds the most recent user `Message` between `SendMessage`
/// and `TurnComplete` so both sides of a turn are available together when
/// `TurnComplete` fires. `assistant_buf` accumulates streaming tokens.
/// `message_history` grows with every completed turn and is passed as the full
/// history to each `save_turn` call so prior turns are not overwritten.
/// Consumers: `run_command_loop` via `CopilotCmdContext`.
pub struct LogState {
    /// Grouped logger, history-adapter, and persistence handles.
    pub handles: LogHandles,
    /// User message for the in-flight turn, set at `SendMessage`, consumed at `TurnComplete`.
    pub pending_user: Option<augur_domain::types::Message>,
    /// Streaming token accumulator; cleared when `TurnComplete` fires.
    pub assistant_buf: OutputText,
    /// Accumulated message records for all completed turns this session.
    /// Passed wholesale to `save_turn` so each save writes the full history.
    pub message_history: Vec<augur_domain::persistence::types::MessageRecord>,
}

fn copilot_endpoint() -> augur_domain::string_newtypes::EndpointName {
    use augur_domain::string_newtypes::{EndpointName, StringNewtype};
    EndpointName::new(COPILOT_ENDPOINT)
}

fn log_tool_event(log: &LogState, content: augur_domain::string_newtypes::OutputText) {
    use augur_domain::newtypes::TimestampMs;
    use augur_domain::types::{Message, Role};

    let msg = Message {
        role: Role::Tool,
        content,
        timestamp: TimestampMs::now(),
        tool_call_id: None,
        tool_calls: None,
    };
    log.handles.history_adapter.record_llm(msg);
}

fn started_tool_content(
    name: augur_domain::ToolName,
    args: serde_json::Value,
) -> augur_domain::string_newtypes::OutputText {
    use augur_domain::string_newtypes::{OutputText, StringNewtype};

    let args_str = serde_json::to_string(&args).unwrap_or_else(|_| "{}".to_owned());
    OutputText::new(format!("[{}:call] {}", name, args_str))
}

fn completed_tool_content(
    name: augur_domain::ToolName,
    success: augur_domain::ExecutionSuccess,
    result: Option<augur_domain::string_newtypes::OutputText>,
) -> augur_domain::string_newtypes::OutputText {
    use augur_domain::string_newtypes::{OutputText, StringNewtype};

    let result_text = result.as_deref().unwrap_or("");
    let status = if success.0 { "ok" } else { "err" };
    OutputText::new(format!("[{}:{}] {}", name, status, result_text))
}

async fn complete_turn(log: &mut LogState) {
    use augur_domain::persistence::types::{MessageRecord, MessageType};
    use augur_domain::types::Message;

    let Some(user_msg) = log.pending_user.take() else {
        return;
    };
    let content = log.assistant_buf.take_all();
    let asst_msg = Message::assistant(content);
    let endpoint = copilot_endpoint();
    log.handles.history_adapter.record_user(user_msg.clone());
    log.handles.history_adapter.record_llm(asst_msg.clone());
    log.message_history.push(MessageRecord {
        message_type: MessageType::User,
        message: user_msg,
    });
    log.message_history.push(MessageRecord {
        message_type: MessageType::Assistant,
        message: asst_msg,
    });
    log.handles
        .persistence
        .save_turn(endpoint, log.message_history.clone())
        .await;
}

async fn persist_error(log: &mut LogState, msg: augur_domain::string_newtypes::OutputText) {
    use augur_domain::newtypes::TimestampMs;
    use augur_domain::persistence::types::{MessageRecord, MessageType};
    use augur_domain::types::{Message, Role};

    let error_record = MessageRecord {
        message_type: MessageType::Error,
        message: Message {
            role: Role::System,
            content: msg,
            timestamp: TimestampMs::now(),
            tool_call_id: None,
            tool_calls: None,
        },
    };
    log.message_history.push(error_record);
    log.handles
        .persistence
        .save_turn(copilot_endpoint(), log.message_history.clone())
        .await;
}

/// Apply a single `AgentOutput` event to the in-flight turn state.
///
/// Accumulates tokens in `log.assistant_buf`. When `TurnComplete` fires,
/// records both sides of the turn to `log.message_history`, logs them via
/// the logger, and saves the full accumulated history to persistence.
///
/// `ToolCallStarted` and `ToolCallCompleted` events are written immediately
/// to the JSONL log as `Role::Tool` messages so tool invocations are captured
/// in the audit log even before the turn completes. The format is:
/// - started:   `[name:call] <args_json>`
/// - completed: `[call_id:ok] <result>` or `[call_id:err] <result>`
///
/// Parameters:
/// - `event`: the `AgentOutput` to process.
/// - `log`: mutable turn state carrying the pending user message, assistant
///   buffer, history vec, logger, and persistence handle.
///
/// Called from the `log_rx` select arm and from `drain_log_events`.
/// Consumers: `run_command_loop`, `drain_log_events`.
#[tracing::instrument(skip(log), level = "debug")]
pub async fn apply_log_event(event: AgentOutput, log: &mut LogState) {
    if handle_buffer_or_tool_event(&event, log) {
        return;
    }
    match event {
        AgentOutput::TurnComplete => complete_turn(log).await,
        AgentOutput::Error(msg) => persist_error(log, msg).await,
        _ => {}
    }
}

fn handle_buffer_or_tool_event(event: &AgentOutput, log: &mut LogState) -> bool {
    match event {
        AgentOutput::Token(token) => {
            log.assistant_buf.push_output(token);
            true
        }
        AgentOutput::ToolCallStarted { name, args } => {
            log_tool_event(log, started_tool_content(name.clone(), args.clone()));
            true
        }
        AgentOutput::ToolCallCompleted {
            name,
            success,
            result,
            ..
        } => {
            log_tool_event(
                log,
                completed_tool_content(name.clone(), *success, result.clone()),
            );
            true
        }
        _ => false,
    }
}

/// Drain all immediately-available events from `log_rx` into `log`.
///
/// Uses `try_recv` to process every event already buffered in the broadcast
/// channel without yielding to the async executor. Called at the start of
/// every `SendMessage` handler so that a `TurnComplete` from the previous
/// turn is committed to `message_history` before `pending_user` and
/// `assistant_buf` are overwritten with the next turn's context.
///
/// This prevents a race where the unbiased outer `select!` picks `SendMessage`
/// before `TurnComplete` when both are ready simultaneously, which would save
/// the wrong user message or drop the prior turn entirely.
///
/// Parameters:
/// - `log_rx`: mutable reference to the broadcast receiver for `AgentOutput`.
/// - `log`: mutable turn state to apply drained events to.
///
/// Consumers: `run_command_loop` `SendMessage` arm.
#[tracing::instrument(skip(log_rx, log), level = "debug")]
pub async fn drain_log_events(
    log_rx: &mut tokio::sync::broadcast::Receiver<AgentOutput>,
    log: &mut LogState,
) {
    loop {
        let should_continue = handle_drain_result(log_rx.try_recv(), log).await;
        if !should_continue {
            break;
        }
    }
}

async fn handle_drain_result(
    recv_result: Result<AgentOutput, tokio::sync::broadcast::error::TryRecvError>,
    log: &mut LogState,
) -> bool {
    match recv_result {
        Ok(event) => {
            apply_log_event(event, log).await;
            true
        }
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
            tracing::warn!(
                n,
                "CopilotChatActor: log receiver lagged while draining, some tokens missed"
            );
            true
        }
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        | Err(tokio::sync::broadcast::error::TryRecvError::Closed) => false,
    }
}
