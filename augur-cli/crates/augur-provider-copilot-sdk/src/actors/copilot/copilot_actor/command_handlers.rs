use super::super::assistant::{
    apply_log_event, build_sdk_attachments, compact_or_shutdown, drain_log_events,
    format_sdk_error, keepalive_session, log_sdk_error, send_or_shutdown, SessionOpOutcome,
};
use super::super::commands::CopilotChatCmd;
use super::runtime_types::{CommandLoopState, CopilotCmdContext, LoopExit};
use augur_domain::string_newtypes::{EndpointName, ModelId, OutputText, StringNewtype};
use augur_domain::types::AgentOutput;

/// Receive one event from the log broadcast channel and apply it to `log`.
///
/// On `RecvError::Lagged` a warning is traced; on `RecvError::Closed` the
/// function returns silently without modifying state.
pub(super) async fn handle_log_output(
    output: Result<AgentOutput, tokio::sync::broadcast::error::RecvError>,
    log: &mut super::super::assistant::LogState,
) {
    use tokio::sync::broadcast::error::RecvError;

    match output {
        Ok(event) => apply_log_event(event, log).await,
        Err(RecvError::Lagged(n)) => {
            tracing::warn!(
                n,
                "CopilotChatActor: log receiver lagged, some tokens missed"
            );
        }
        Err(RecvError::Closed) => {}
    }
}

/// Clear transient per-turn log fields so the next turn starts from a clean slate.
///
/// Sets `pending_user` to `None` and resets `assistant_buf` to an empty string.
pub(super) fn reset_log_state(log: &mut super::super::assistant::LogState) {
    log.pending_user = None;
    log.assistant_buf = OutputText::from("");
}

fn restore_message_history(
    log: &mut super::super::assistant::LogState,
    records: Vec<augur_domain::persistence::types::MessageRecord>,
) {
    log.message_history = records;
    tracing::debug!(
        count = log.message_history.len(),
        "CopilotChatActor: message_history seeded from restored session"
    );
}

async fn persist_model_switch(log: &mut super::super::assistant::LogState, model_id: &ModelId) {
    use augur_domain::persistence::types::{MessageRecord, MessageType};
    use augur_domain::types::Message;

    if log.message_history.is_empty() {
        return;
    }
    log.message_history.push(MessageRecord {
        message_type: MessageType::System,
        message: Message::system(OutputText::new(format!(
            "[system] model switched to {model_id}"
        ))),
    });
    log.handles
        .persistence
        .save_turn(EndpointName::new("copilot"), log.message_history.clone())
        .await;
}

async fn handle_send_message(
    state: CommandLoopState<'_, '_>,
    text: augur_domain::PromptText,
    attachments: Vec<augur_domain::string_newtypes::FilePath>,
) -> Option<LoopExit> {
    let CommandLoopState {
        session,
        ctx,
        log_rx,
    } = state;
    drain_log_events(log_rx, &mut ctx.log).await;
    ctx.log.pending_user = Some(augur_domain::types::Message::user(text.as_str()));
    ctx.log.assistant_buf = OutputText::from("");
    let options = copilot_sdk::MessageOptions {
        prompt: text.into_inner(),
        attachments: Some(build_sdk_attachments(&attachments)),
        mode: None,
    };
    match send_or_shutdown(session, options, ctx.cmd_rx).await {
        SessionOpOutcome::Done => None,
        SessionOpOutcome::Shutdown => Some(LoopExit::Clean),
        SessionOpOutcome::Error(error) => {
            log_sdk_error(
                &error,
                &OutputText::from("CopilotChatActor: send failed, session may be dead"),
            );
            super::emit(AgentOutput::Error(format_sdk_error(&error)), ctx.output_tx);
            Some(LoopExit::FatalError)
        }
    }
}

async fn handle_compact(state: CommandLoopState<'_, '_>) -> Option<LoopExit> {
    let CommandLoopState {
        session,
        ctx,
        log_rx,
    } = state;
    drain_log_events(log_rx, &mut ctx.log).await;
    reset_log_state(&mut ctx.log);
    match compact_or_shutdown(session, ctx.cmd_rx).await {
        SessionOpOutcome::Done => None,
        SessionOpOutcome::Shutdown => Some(LoopExit::Clean),
        SessionOpOutcome::Error(error) => {
            log_sdk_error(
                &error,
                &OutputText::from("CopilotChatActor: compact failed"),
            );
            super::emit(AgentOutput::Error(format_sdk_error(&error)), ctx.output_tx);
            None
        }
    }
}

async fn handle_set_model(
    state: CommandLoopState<'_, '_>,
    model_id: ModelId,
    reasoning_effort: Option<augur_domain::thinking_mode::ReasoningEffort>,
) {
    let CommandLoopState {
        session,
        ctx,
        log_rx,
    } = state;
    drain_log_events(log_rx, &mut ctx.log).await;
    let opts = reasoning_effort.map(|e| copilot_sdk::SetModelOptions {
        reasoning_effort: Some(e.as_ref().to_owned()),
    });
    if let Err(error) = session.set_model(model_id.as_str(), opts).await {
        tracing::warn!(
            error = %error,
            model_id = %model_id,
            "CopilotChatActor: set_model failed"
        );
        return;
    }
    super::emit(
        AgentOutput::ActiveModelChanged(model_id.clone()),
        ctx.output_tx,
    );
    persist_model_switch(&mut ctx.log, &model_id).await;
}

fn spawn_background_agent(
    ctx: &CopilotCmdContext<'_>,
    agent: augur_domain::string_newtypes::AgentName,
    prompt: augur_domain::string_newtypes::PromptText,
) {
    use crate::actors::copilot::background_agent::{
        run_background_agent, BackgroundAgentArgs, BackgroundAgentConfig,
    };
    use crate::actors::copilot::event_classifier::CopilotEventClassifier;
    let feed_id = augur_domain::types::FeedId::Agent(
        augur_domain::string_newtypes::ToolCallId::from(uuid::Uuid::new_v4().to_string()),
    );

    tokio::spawn(run_background_agent(
        BackgroundAgentArgs::builder()
            .config(
                BackgroundAgentConfig::builder()
                    .agent(agent)
                    .feed_id(feed_id)
                    .prompt(prompt)
                    .build(),
            )
            .feed_tx(ctx.dispatch.agent_feed_tx.clone())
            .maybe_token_tracker(Some(ctx.dispatch.token_tracker.clone()))
            .classifier(std::sync::Arc::new(CopilotEventClassifier))
            .build(),
    ));
}

/// Dispatch a single `CopilotChatCmd` inside the active command loop.
///
/// Returns `Some(LoopExit)` to terminate the loop or `None` to continue.
pub(super) async fn handle_loop_command(
    state: CommandLoopState<'_, '_>,
    cmd: CopilotChatCmd,
) -> Option<LoopExit> {
    match cmd {
        CopilotChatCmd::SendMessage { text, attachments } => {
            handle_send_message(state, text, attachments).await
        }
        CopilotChatCmd::Compact => handle_compact(state).await,
        CopilotChatCmd::Restore(records) => {
            restore_message_history(&mut state.ctx.log, records);
            None
        }
        CopilotChatCmd::SetModel {
            model_id,
            reasoning_effort,
        } => {
            handle_set_model(state, model_id, reasoning_effort).await;
            None
        }
        CopilotChatCmd::ReplaceSession { sdk_session_id } => {
            state.ctx.log.message_history.clear();
            reset_log_state(&mut state.ctx.log);
            Some(LoopExit::ReplaceSession(sdk_session_id))
        }
        CopilotChatCmd::RunBackgroundAgent { agent, prompt } => {
            spawn_background_agent(state.ctx, agent, prompt);
            None
        }
        CopilotChatCmd::Shutdown => Some(LoopExit::Clean),
    }
}

/// Send a keepalive ping to the SDK session and emit a system message if the session is dead.
///
/// Returns `Some(LoopExit::FatalError)` when the session has expired so the
/// caller can restart; returns `None` when the session is still alive.
pub(super) async fn handle_keepalive_tick(
    session: &copilot_sdk::Session,
    output_tx: &tokio::sync::broadcast::Sender<AgentOutput>,
) -> Option<LoopExit> {
    if matches!(
        keepalive_session(session).await,
        augur_domain::types::SessionAliveness::Dead
    ) {
        super::emit(
            AgentOutput::SystemMessage(OutputText::new(
                "Session expired during idle period. Restarting session - previous context has been reset.".to_owned(),
            )),
            output_tx,
        );
        return Some(LoopExit::FatalError);
    }
    None
}
