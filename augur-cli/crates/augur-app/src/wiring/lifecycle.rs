use super::RunRuntime;

/// Sends shutdown signals to all runtime actors in layer-aware order.
///
/// Shutdown proceeds in reverse dependency order to ensure clean termination:
///
/// **UI Layer** (optional): Shuts down chat provider and optional supervisor.
///
/// **Domain Layer**: Shuts down agent, session, file scanner, guided plan,
/// and optional ask actor.
///
/// **Infrastructure Layer** (shutdown last): Shuts down tool, file read,
/// logger, LLM, and optional cache actors.
///
/// This ordering ensures that higher-level actors (which may be awaiting
/// responses from lower-level actors) are terminated before the infrastructure
/// they depend on is shut down, preventing deadlocks or orphaned tasks.
pub fn shutdown_runtime(runtime: &RunRuntime) {
    // Shutdown UI layer first (reverse dependency order)
    runtime.app.handles.optional.chat_provider.shutdown();
    if let Some(supervisor) = runtime.app.handles.optional.supervisor.as_ref() {
        supervisor.shutdown();
    }

    // Shutdown domain layer
    runtime.app.handles.primary.domain.agent.shutdown();
    runtime.app.handles.primary.domain.session.shutdown();
    runtime.app.handles.primary.domain.file_scanner.shutdown();
    runtime.app.handles.primary.domain.guided_plan.shutdown();
    runtime
        .app
        .handles
        .primary
        .domain
        .deterministic_orchestrator
        .shutdown();
    let _ = runtime
        .core
        .context
        .control
        .openrouter_orchestrator_handle
        .shutdown();
    runtime.app.handles.optional.ask_shutdown.shutdown();

    // Shutdown infrastructure layer last
    runtime.core.handles.services.tool.shutdown();
    runtime.core.handles.services.file_read.shutdown();
    // Kill the LSP child process deterministically. After every send-side
    // reference has been dropped (tool and openrouter-orchestrator shutdown
    // above), signal the LSP actor to kill rust-analyzer and exit so the
    // join handle resolves immediately rather than deadlocking on mpsc
    // ordering.
    runtime.core.context.control.lsp_handle.kill();
    runtime.core.handles.io.logger.shutdown();
    runtime.core.handles.services.llm.shutdown();
    if let Some(cache) = runtime.core.handles.cache.as_ref() {
        cache.shutdown();
    }
    runtime.core.context.startup.token_tracker.shutdown();
    // Shutdown feed-consumer actors; dropping the handles is sufficient but
    // explicit shutdown gives the actors a chance to flush any in-flight items.
    runtime.app.handles.optional.consumers.llm_feed.shutdown();
    runtime
        .app
        .handles
        .optional
        .consumers
        .user_message
        .shutdown();
}

/// Block until all spawned actor tasks have completed.
///
/// Awaits actors in layer order: UI first, then domain, then optional
/// domain/UI layers (ask tool, Copilot, executor), and finally infrastructure.
/// Called from `crate::wiring::run` after [`shutdown_runtime`] signals all actors to stop.
pub async fn await_runtime(runtime: RunRuntime) {
    // Await UI layer first
    let _ = runtime.app.joins.primary.ui.tui.await;

    // Await domain layer
    let _ = tokio::join!(
        runtime.app.joins.primary.domain.agent,
        runtime.app.joins.primary.domain.session,
        runtime.app.joins.primary.domain.deterministic_orchestrator,
        runtime.app.joins.primary.domain.file_scanner,
        runtime.app.joins.primary.domain.ask_agent
    );

    // Await optional domain layer (ask tool)
    if let Some(join) = runtime.app.joins.optional.ask_tool {
        let _ = join.await;
    }

    // Await UI-layer optionals (copilot/executor)
    if let Some(join) = runtime.app.joins.optional.copilot {
        let _ = join.await;
    }
    if let Some(join) = runtime.app.joins.optional.executor {
        let _ = join.await;
    }

    // Drop the LSP-actors mpsc sender so the drain_requests loop can exit.
    // kill() was already called in shutdown_runtime, but the shutdown_lsp_handle
    // clone in CoreRuntime::CoreControl::lsp_handle keeps the channel open.
    // Without this drop, drain_requests in the LSP actor blocks forever on
    // rx.recv() waiting for a sender that lives inside Runtime (which is not
    // dropped until await_runtime returns).
    drop(runtime.core.context.control.lsp_handle);

    // Await infrastructure layer last
    let _ = tokio::join!(
        runtime.core.actor_joins.tool,
        runtime.core.actor_joins.file_read,
        runtime.core.support_joins.logger,
        runtime.core.actor_joins.llm,
        runtime.core.support_joins.token_tracker,
        runtime.core.support_joins.lsp,
    );
}
