use super::super::assistant::{register_query_user_tool, start_event_dispatch, EventDispatchArgs};
use super::super::feed_router::FeedChannels;
use super::runtime_types::{
    ActivateSessionArgs, ActiveSessionCommandContextArgs, CommandContextArgs, CopilotCmdContext,
    InitialSessionState, StartActiveSessionLifecycleArgs,
};
use super::session_lifecycle::run_session_lifecycle;
use augur_domain::string_newtypes::{SdkSessionId, StringNewtype};

/// Finalize session activation: persist the SDK session ID, log the event, register the query-user tool, and start event dispatch.
///
/// Called once after a session is successfully created or restarted.
pub(super) async fn activate_session(args: ActivateSessionArgs<'_, '_>) {
    let ActivateSessionArgs {
        session,
        ctx,
        reason,
    } = args;
    ctx.log
        .handles
        .persistence
        .set_sdk_session_id(SdkSessionId::new(session.session_id()));
    tracing::info!(
        sdk_session_id = session.session_id(),
        session_action = reason,
        "CopilotChatActor: session active"
    );
    register_query_user_tool(session, ctx.dispatch.query_tx.clone()).await;
    start_event_dispatch(
        session,
        EventDispatchArgs::builder()
            .output_tx(ctx.output_tx.clone())
            .feed_channels(FeedChannels::single(ctx.dispatch.agent_feed_tx.clone()))
            .token_tracker(ctx.dispatch.token_tracker.clone())
            .build(),
    );
}

/// Construct a `CopilotCmdContext` from `CommandContextArgs`, seeding message history from any pending restore.
///
/// Returns the initial `Arc<Session>` alongside the context so the caller can
/// proceed directly to the command loop.
pub(super) fn build_command_context(
    args: CommandContextArgs<'_>,
) -> (std::sync::Arc<copilot_sdk::Session>, CopilotCmdContext<'_>) {
    let CommandContextArgs {
        cmd_rx,
        output_tx,
        dispatch,
        initial_state,
    } = args;
    let InitialSessionState {
        session,
        log,
        pending_restore,
    } = initial_state;
    let mut ctx = CopilotCmdContext::builder()
        .cmd_rx(cmd_rx)
        .output_tx(output_tx)
        .log(log)
        .dispatch(dispatch)
        .build();
    if !pending_restore.is_empty() {
        ctx.log.message_history = pending_restore;
    }
    (session, ctx)
}

async fn activate_initial_session(
    session: &std::sync::Arc<copilot_sdk::Session>,
    ctx: &CopilotCmdContext<'_>,
) {
    activate_session(
        ActivateSessionArgs::builder()
            .session(session)
            .ctx(ctx)
            .reason("established")
            .build(),
    )
    .await;
}

/// Build a command context from an `ActiveSessionCommandContextArgs` bundle.
///
/// Thin adapter over [`build_command_context`] for callers that hold
/// `ActiveSessionCommandContextArgs` rather than `CommandContextArgs` directly.
pub(super) fn build_active_session_command_context<'a>(
    args: ActiveSessionCommandContextArgs<'a>,
) -> (std::sync::Arc<copilot_sdk::Session>, CopilotCmdContext<'a>) {
    let ActiveSessionCommandContextArgs {
        cmd_rx,
        output_tx,
        dispatch,
        initial_state,
    } = args;
    build_command_context(
        CommandContextArgs::builder()
            .cmd_rx(cmd_rx)
            .output_tx(output_tx)
            .dispatch(dispatch)
            .initial_state(initial_state)
            .build(),
    )
}

/// Activate the initial session and enter the session lifecycle loop.
///
/// Calls `activate_session` for the initial `Session`, then delegates to
/// `run_session_lifecycle` which handles restarts and session replacements.
pub(super) async fn start_active_session_lifecycle(args: StartActiveSessionLifecycleArgs<'_, '_>) {
    let StartActiveSessionLifecycleArgs {
        client,
        config,
        initial_session,
        ctx,
    } = args;
    activate_initial_session(&initial_session, ctx).await;
    run_session_lifecycle(
        super::runtime_types::SessionLifecycleArgs::builder()
            .client(client)
            .config(config)
            .initial_session(initial_session)
            .ctx(ctx)
            .build(),
    )
    .await;
}
