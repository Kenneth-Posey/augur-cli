use super::super::assistant::{
    check_auth_status, create_session, query_user_tool_def, CreateOrResumeSessionArgs,
};
use super::runtime_types::{
    LoopExit, ReplaceSessionArgs, ResolveLoopExitArgs, RestartSessionArgs, SessionLifecycleArgs,
};
use augur_domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::types::AgentOutput;

/// Drive the session lifecycle: run the command loop and restart or replace the session as directed by `LoopExit` signals.
///
/// Loops until the session terminates cleanly or a restart/replace cycle
/// exhausts its retry budget.
pub(super) async fn run_session_lifecycle(args: SessionLifecycleArgs<'_, '_>) {
    let SessionLifecycleArgs {
        client,
        config,
        initial_session,
        ctx,
    } = args;
    let mut current_session = initial_session;
    let mut restart_tried = false;
    loop {
        let exit = super::command_loop::run_command_loop(&current_session, ctx).await;
        let Some(next_session) = resolve_loop_exit(
            ResolveLoopExitArgs::builder()
                .client(client)
                .config(config)
                .exit(exit)
                .restart_tried(&mut restart_tried)
                .ctx(ctx)
                .build(),
        )
        .await
        else {
            break;
        };
        current_session = next_session;
    }
}

async fn resolve_loop_exit(
    args: ResolveLoopExitArgs<'_, '_>,
) -> Option<std::sync::Arc<copilot_sdk::Session>> {
    let ResolveLoopExitArgs {
        client,
        config,
        exit,
        restart_tried,
        ctx,
    } = args;
    match exit {
        LoopExit::Clean => None,
        LoopExit::FatalError => {
            restart_session_after_failure(
                RestartSessionArgs::builder()
                    .client(client)
                    .config(config)
                    .restart_tried(restart_tried)
                    .ctx(ctx)
                    .build(),
            )
            .await
        }
        LoopExit::ReplaceSession(sdk_id) => {
            *restart_tried = false;
            replace_session(
                ReplaceSessionArgs::builder()
                    .client(client)
                    .config(config)
                    .maybe_sdk_id(sdk_id)
                    .ctx(ctx)
                    .build(),
            )
            .await
        }
    }
}

async fn restart_session_after_failure(
    args: RestartSessionArgs<'_, '_>,
) -> Option<std::sync::Arc<copilot_sdk::Session>> {
    let RestartSessionArgs {
        client,
        config,
        restart_tried,
        ctx,
    } = args;
    if *restart_tried {
        tracing::warn!("CopilotChatActor: restarted session also failed, giving up");
        return None;
    }
    *restart_tried = true;
    tracing::warn!("CopilotChatActor: attempting session restart");
    if let Some(error) = check_auth_status(client).await {
        super::emit(error, ctx.output_tx);
        return None;
    }
    match create_session(client, config, vec![query_user_tool_def()]).await {
        Ok(session) => {
            super::session_activation::activate_session(
                super::runtime_types::ActivateSessionArgs::builder()
                    .session(&session)
                    .ctx(&*ctx)
                    .reason("restarted")
                    .build(),
            )
            .await;
            Some(session)
        }
        Err(error) => {
            tracing::error!(error = %error, "CopilotChatActor: session restart failed");
            super::emit(
                AgentOutput::Error(OutputText::new(format!(
                    "Session restart failed: {}",
                    error
                ))),
                ctx.output_tx,
            );
            None
        }
    }
}

async fn replace_session(
    args: ReplaceSessionArgs<'_, '_>,
) -> Option<std::sync::Arc<copilot_sdk::Session>> {
    let ReplaceSessionArgs {
        client,
        config,
        sdk_id,
        ctx,
    } = args;
    match super::startup::create_or_emit_session(
        CreateOrResumeSessionArgs::builder()
            .client(client)
            .config(config)
            .tools(vec![query_user_tool_def()])
            .maybe_sdk_session_id(sdk_id)
            .build(),
        ctx.output_tx,
    )
    .await
    {
        Some(session) => {
            super::session_activation::activate_session(
                super::runtime_types::ActivateSessionArgs::builder()
                    .session(&session)
                    .ctx(&*ctx)
                    .reason("replaced")
                    .build(),
            )
            .await;
            Some(session)
        }
        None => None,
    }
}
