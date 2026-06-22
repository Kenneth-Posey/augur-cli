use super::super::assistant::{
    CreateOrResumeSessionArgs, LogHandles, LogState, build_client, check_auth_status,
    create_or_resume_session, query_user_tool_def,
};
use super::super::commands::CopilotChatCmd;
use super::runtime_types::{
    ActiveSessionCommandContextArgs, InitialSessionInputs, InitialSessionServices,
    InitialSessionState, RunInitialStateInputs, StartActiveSessionLifecycleArgs,
};
use super::session_activation::{
    build_active_session_command_context, start_active_session_lifecycle,
};
use augur_domain::config::types::CopilotChatConfig;
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::string_newtypes::{ModelId, ModelLabel, OutputText, SdkSessionId, StringNewtype};
use augur_domain::types::{AgentOutput, ModelOption};
use augur_domain::{HistoryAdapterHandle, LoggerHandle};
use tokio::sync::{broadcast, mpsc};

/// Construct the initial `LogState` from the provided logger, persistence, and history-adapter handles.
///
/// The returned state has an empty message history and a zero-length assistant buffer.
pub(super) fn build_log_state(
    logger: LoggerHandle,
    persistence: PersistenceHandle,
    history_adapter: HistoryAdapterHandle,
) -> LogState {
    LogState::builder()
        .handles(
            LogHandles::builder()
                .logger(logger)
                .persistence(persistence)
                .history_adapter(history_adapter)
                .build(),
        )
        .assistant_buf(OutputText::from(""))
        .message_history(Vec::new())
        .build()
}

/// Build and start the Copilot SDK client, verify auth, and return it if successful.
///
/// Emits `AgentOutput::Error` and returns `None` on any construction, startup,
/// or auth failure so the caller can exit cleanly without panicking.
pub(super) async fn start_sdk_client(
    config: &CopilotChatConfig,
    output_tx: &broadcast::Sender<AgentOutput>,
) -> Option<copilot_sdk::Client> {
    let client = match build_client(config) {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(error = %error, "CopilotChatActor: failed to build SDK client");
            super::emit(
                AgentOutput::Error(OutputText::new(error.to_string())),
                output_tx,
            );
            return None;
        }
    };
    if let Err(error) = client.start().await {
        tracing::error!(error = %error, "CopilotChatActor: failed to start SDK client");
        super::emit(
            AgentOutput::Error(OutputText::new(error.to_string())),
            output_tx,
        );
        return None;
    }
    let protocol_version = client.negotiated_protocol_version().await;
    tracing::warn!(protocol_version = ?protocol_version, "CopilotChatActor: SDK client started");
    if let Some(error) = check_auth_status(&client).await {
        super::emit(error, output_tx);
        let _ = client.stop().await;
        return None;
    }
    Some(client)
}

/// Create or resume a Copilot session, emitting an error event and returning `None` on failure.
///
/// Wraps `create_or_resume_session` with broadcast-channel error emission so
/// callers can treat `None` as a terminal shutdown signal.
pub(super) async fn create_or_emit_session(
    args: CreateOrResumeSessionArgs<'_>,
    output_tx: &broadcast::Sender<AgentOutput>,
) -> Option<std::sync::Arc<copilot_sdk::Session>> {
    match create_or_resume_session(args).await {
        Ok(session) => Some(session),
        Err(error) => {
            tracing::error!(error = %error, "CopilotChatActor: session init failed");
            super::emit(
                AgentOutput::Error(OutputText::new(error.to_string())),
                output_tx,
            );
            None
        }
    }
}

/// Wait for the TUI session signal and create the initial SDK session.
///
/// Emits available models, drains the command channel until a `ReplaceSession`
/// signal arrives, then calls `create_or_emit_session`. Returns `None` on
/// shutdown or session creation failure.
pub(super) async fn initialize_initial_session(
    inputs: InitialSessionInputs<'_>,
    services: InitialSessionServices<'_>,
) -> Option<InitialSessionState> {
    let InitialSessionInputs {
        client,
        config,
        output_tx,
        cmd_rx,
    } = inputs;
    let InitialSessionServices {
        logger,
        persistence,
        history_adapter,
        token_tracker: _token_tracker,
    } = services;
    emit_models_available(client, output_tx).await;

    let (initial_sdk_id, pending_restore) = match wait_for_session_signal(cmd_rx).await {
        Some(result) => result,
        None => {
            tracing::info!("CopilotChatActor: shutdown before session signal");
            return None;
        }
    };

    let session = create_or_emit_session(
        CreateOrResumeSessionArgs::builder()
            .client(client)
            .config(config)
            .tools(vec![query_user_tool_def()])
            .maybe_sdk_session_id(initial_sdk_id)
            .build(),
        output_tx,
    )
    .await?;

    Some(
        InitialSessionState::builder()
            .session(session)
            .log(build_log_state(logger, persistence, history_adapter))
            .pending_restore(pending_restore)
            .build(),
    )
}

/// Orchestrate the active session lifecycle from initial state to completion.
///
/// Builds the initial session state, assembles the dispatch handles and command
/// context, then delegates to `start_active_session_lifecycle`.
pub(super) async fn run_active_session(
    client: &copilot_sdk::Client,
    config: &CopilotChatConfig,
    args: super::runtime_types::RunArgs,
) {
    let super::runtime_types::RunArgs {
        mut cmd_rx,
        output_tx,
        handles:
            super::runtime_types::RunHandles {
                logger,
                persistence,
                history_adapter,
            },
        channels,
    } = args;
    let token_tracker = channels.token_tracker;
    let Some(initial_state) = initialize_run_initial_state(
        RunInitialStateInputs {
            client,
            config,
            services: super::runtime_types::RunInitialServices {
                logger,
                persistence,
                history_adapter,
                token_tracker: &token_tracker,
            },
        },
        &output_tx,
        &mut cmd_rx,
    )
    .await
    else {
        return;
    };
    let dispatch = super::runtime_types::CopilotDispatchHandles {
        query_tx: channels.query_tx,
        agent_feed_tx: channels.agent_feed_tx,
        token_tracker,
    };
    let (initial_session, mut ctx) =
        build_active_session_command_context(ActiveSessionCommandContextArgs {
            cmd_rx: &mut cmd_rx,
            output_tx: &output_tx,
            dispatch,
            initial_state,
        });
    start_active_session_lifecycle(StartActiveSessionLifecycleArgs {
        client,
        config,
        initial_session,
        ctx: &mut ctx,
    })
    .await;
}

/// Initialize the `InitialSessionState` needed before entering the active session loop.
///
/// Adapts `RunInitialStateInputs` into the flat `InitialSessionInputs` /
/// `InitialSessionServices` split and delegates to `initialize_initial_session`.
pub(super) async fn initialize_run_initial_state(
    inputs: RunInitialStateInputs<'_>,
    output_tx: &broadcast::Sender<AgentOutput>,
    cmd_rx: &mut mpsc::Receiver<CopilotChatCmd>,
) -> Option<InitialSessionState> {
    initialize_initial_session(
        InitialSessionInputs::builder()
            .client(inputs.client)
            .config(inputs.config)
            .output_tx(output_tx)
            .cmd_rx(cmd_rx)
            .build(),
        InitialSessionServices::builder()
            .logger(inputs.services.logger)
            .persistence(inputs.services.persistence)
            .history_adapter(inputs.services.history_adapter)
            .token_tracker(inputs.services.token_tracker)
            .build(),
    )
    .await
}

/// Wait for the TUI picker to signal which SDK session to start.
///
/// Drains `cmd_rx` until a `ReplaceSession` command arrives. Returns
/// `Some((sdk_session_id, restored_records))` where `sdk_session_id` is
/// `None` for a fresh session or `Some(id)` for a resumed session, and
/// `restored_records` holds any message history sent via a preceding `Restore`
/// command (from `apply_restored_session`). Returns `None` when the channel
/// closes or `Shutdown` arrives before a session signal - the caller should
/// exit without creating a session.
///
/// `SendMessage`, `Compact`, and `SetModel` cannot arrive before the TUI picker
/// resolves (they are only reachable from Chat mode, which requires picker
/// resolution first). They are logged at WARN and dropped defensively.
///
/// Consumers: `run_with_sdk` before initial session creation.
async fn wait_for_session_signal(
    cmd_rx: &mut mpsc::Receiver<CopilotChatCmd>,
) -> Option<(
    Option<SdkSessionId>,
    Vec<augur_domain::persistence::types::MessageRecord>,
)> {
    let mut pending_restore = Vec::new();
    loop {
        match cmd_rx.recv().await? {
            CopilotChatCmd::Restore(records) => {
                pending_restore = records;
            }
            CopilotChatCmd::ReplaceSession { sdk_session_id } => {
                return Some((sdk_session_id, pending_restore));
            }
            CopilotChatCmd::Shutdown => return None,
            _ => {
                tracing::warn!(
                    "CopilotChatActor: unexpected command before session signal; dropped"
                );
            }
        }
    }
}

/// Fetch the list of available models from the SDK client and emit `ModelsAvailable`.
///
/// Calls `client.list_models()` which is cached after the first call. Converts
/// each `ModelInfo` into a `ModelOption` using the `name` field as display name
/// and `billing.multiplier` (0.0 when absent). On failure, logs a warning and
/// emits nothing so startup continues without model picker data.
///
/// Consumers: `run_with_sdk` immediately after session creation.
async fn emit_models_available(
    client: &copilot_sdk::Client,
    output_tx: &broadcast::Sender<AgentOutput>,
) {
    match client.list_models().await {
        Ok(models) => {
            let options: Vec<ModelOption> = models
                .into_iter()
                .map(|m| {
                    ModelOption::builder()
                        .id(ModelId::new(&m.id))
                        .display_name(ModelLabel::new(&m.name))
                        .build()
                })
                .collect();
            super::emit(AgentOutput::ModelsAvailable(options), output_tx);
        }
        Err(e) => {
            tracing::warn!(error = %e, "CopilotChatActor: list_models failed, /model picker unavailable");
        }
    }
}
