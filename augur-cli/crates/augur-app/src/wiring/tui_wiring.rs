use super::{
    ActorRuntime, ConsumerHandles, CoreRuntime, SpawnedTuiDeps, TaskJoin, TuiBuildChannels,
    TuiBuildCore, TuiChannelDeps, TuiChannels, TuiProviders, TuiRuntimeDeps, TuiRuntimeInput,
    TuiServiceDeps, TuiStartupDeps,
};
use augur_core::actors;
use augur_core::actors::llm_feed_consumer::llm_feed_consumer_actor::spawn as spawn_llm_feed_consumer;
use augur_core::actors::llm_feed_consumer::llm_feed_consumer_ops::LlmFeedOutputChannels;
use augur_core::actors::user_message_consumer::user_message_consumer_actor::{
    UserMessageOutputChannels, spawn as spawn_user_msg_consumer,
};
use augur_domain::domain::feeds::{LlmFeedMessage, UserFeedMessage};
use augur_domain::domain::newtypes::Count;
use augur_domain::domain::types::{AgentFeedOutput, AgentOutput, StreamChunk};
use augur_tui::actors::tui::tui_actor::{
    TuiInputChannels, TuiOverlayHandles, TuiServiceHandles, TuiServiceTools, TuiStartupData,
    TuiSubActorHandles,
};
use augur_tui::actors::tui_agent_panel::TuiAgentPanelHandle;
use augur_tui::actors::tui_agent_panel::tui_agent_panel_actor::{
    TuiAgentPanelConfig, spawn as spawn_agent_panel,
};
use augur_tui::actors::tui_ask_panel::tui_ask_panel_actor::spawn as spawn_ask_panel;
use augur_tui::actors::tui_chat_menu::tui_chat_menu_actor::spawn as spawn_chat_menu;
use augur_tui::actors::tui_dynamic_controls::tui_dynamic_controls_actor::spawn as spawn_controls;
use augur_tui::actors::tui_main_feed_panel::TuiMainFeedPanelHandle;
use augur_tui::actors::tui_main_feed_panel::tui_main_feed_panel_actor::{
    TuiMainFeedConfig, spawn as spawn_main_feed,
};
use augur_tui::actors::tui_main_feed_panel::tui_main_feed_panel_ops::MainFeedItem;
use augur_tui::actors::tui_spinner::tui_spinner_actor::spawn as spawn_spinner;
use tokio::sync::mpsc;

/// Decompose raw wiring inputs into a [`SpawnedTuiDeps`] bundle.
///
/// Extracts startup data (config, renderer, orchestrator handle) from `core`,
/// builds service dependencies, and repackages the channel arguments into
/// [`TuiChannelDeps`]. The resulting bundle is consumed by [`build_tui_deps`]
/// to produce the final [`TuiRuntimeDeps`].
pub fn build_spawned_tui_deps(
    core: TuiBuildCore<'_>,
    channels: TuiBuildChannels,
) -> SpawnedTuiDeps {
    let startup = TuiStartupDeps {
        config: core.config.clone(),
        renderer: core.renderer,
        orchestrator: core.domain.deterministic_orchestrator.handle.clone(),
    };
    let services = TuiServiceDeps {
        chat_provider: core.chat_provider,
        session: core.domain.session.handle.clone(),
        ask: core.domain.ask.handle.clone(),
        file_scanner: core.planning.file_scanner.handle.clone(),
        guided_plan: core.planning.guided_plan.clone(),
    };
    let channels = TuiChannelDeps {
        output_rx: channels.output_rx,
        supervisor: channels.supervisor_rx,
        feed_tx: channels.feed_tx,
        feed_rx: channels.feed_rx,
    };
    SpawnedTuiDeps {
        startup,
        services,
        channels,
    }
}

/// Combine [`TuiStartupDeps`], [`TuiServiceDeps`], and [`TuiChannelDeps`] into [`TuiRuntimeDeps`].
///
/// Thin constructor that avoids passing three separate structs to
/// [`spawn_tui_actor`]. No allocation or cloning occurs beyond what the
/// field assignments imply.
pub fn build_tui_deps(
    startup: TuiStartupDeps,
    services: TuiServiceDeps,
    channels: TuiChannelDeps,
) -> TuiRuntimeDeps {
    TuiRuntimeDeps {
        startup,
        services,
        channels,
    }
}

/// Expand [`TuiRuntimeDeps`] into the [`TuiRuntimeInput`] expected by the TUI spawn function.
///
/// Takes the query receiver from `core` via [`take_query_rx`] (consuming it)
/// and organises all providers and channels into the nested `\`TuiProviders\``
/// and `\`TuiChannels\`` structures. Called once; the query receiver must not
/// have been taken previously.
pub fn build_tui_runtime_deps(
    core: &mut CoreRuntime,
    deps: TuiRuntimeDeps,
    sub_actors: TuiSubActorHandles,
) -> TuiRuntimeInput {
    TuiRuntimeInput {
        config: deps.startup.config,
        renderer: deps.startup.renderer,
        providers: TuiProviders {
            chat: deps.services.chat_provider,
            session: deps.services.session,
            orchestrator: deps.startup.orchestrator,
            tools: TuiServiceTools::builder()
                .command(core.context.control.command.clone())
                .file_scanner(deps.services.file_scanner)
                .guided_plan(deps.services.guided_plan)
                .ask(deps.services.ask)
                .logger(core.handles.io.logger.clone())
                .build(),
        },
        channels: TuiChannels {
            output: deps.channels.output_rx,
            query: take_query_rx(core),
            supervisor: deps.channels.supervisor,
            feed_tx: deps.channels.feed_tx,
            feed_rx: deps.channels.feed_rx,
        },
        sub_actors,
    }
}

/// Build [`TuiRuntimeInput`] and spawn the TUI actor, returning its [`ActorRuntime`].
///
/// Delegates to [`build_tui_runtime_deps`] to finalise channel wiring, then
/// calls [`spawn_tui_runtime`] and wraps the result with [`super::actor_runtime`].
/// `core` is mutably borrowed to consume the query receiver exactly once.
pub fn spawn_tui_actor(
    core: &mut CoreRuntime,
    deps: TuiRuntimeDeps,
    sub_actors: TuiSubActorHandles,
) -> ActorRuntime<augur_tui::TuiHandle> {
    let input = build_tui_runtime_deps(core, deps, sub_actors);
    super::actor_runtime(spawn_tui_runtime(core, input))
}

/// Assemble `\`TuiSpawnArgs\`` from `input` and `core` and spawn the TUI actor.
///
/// Populates provider handles (agent/chat, session, tools, orchestrator,
/// catalog manager), input channels (output broadcast, query, supervisor),
/// and startup data (session summaries, persistence, token tracker, config,
/// renderer). Returns the raw `(TaskJoin, TuiHandle)` pair.
pub fn spawn_tui_runtime(
    core: &CoreRuntime,
    input: TuiRuntimeInput,
) -> (TaskJoin, augur_tui::TuiHandle) {
    let spawn_args = augur_tui::actors::tui::tui_actor::TuiSpawnArgs::builder()
        .providers(
            TuiServiceHandles::builder()
                .agent(input.providers.chat)
                .session(input.providers.session)
                .tools(input.providers.tools)
                .orchestrator(input.providers.orchestrator)
                .catalog_manager(core.handles.catalog_manager.clone())
                .build(),
        )
        .channels(
            TuiInputChannels::builder()
                .output_rx(input.channels.output)
                .query_rx(input.channels.query)
                .maybe_supervisor_rx(input.channels.supervisor)
                .build(),
        )
        .startup(
            TuiStartupData::builder()
                .session_summaries(core.context.startup.session_summaries.clone())
                .persistence(core.context.startup.persistence.clone())
                .token_tracker(core.context.startup.token_tracker.clone())
                .config(input.config)
                .renderer(input.renderer)
                .build(),
        )
        .sub_actors(input.sub_actors)
        .build();
    augur_tui::actors::tui::tui_actor::spawn(
        spawn_args,
        input.channels.feed_tx,
        input.channels.feed_rx,
    )
}

/// Spawn the TUI sub-actors with drop-sink channels.
///
/// The agent-panel and main-feed actors forward events to a `unified_tx` sink.
/// Here the unified receivers are discarded; the actors silently ignore send
/// errors. The ask-panel actor is spawned with a capacity of 8.
pub fn spawn_tui_sub_actors() -> TuiSubActorHandles {
    let (agent_feed_tx, _) = mpsc::channel::<AgentFeedOutput>(8);
    let (main_feed_tx, _) = mpsc::channel::<MainFeedItem>(8);

    let (_, agent_panel) = spawn_agent_panel(TuiAgentPanelConfig {
        unified_tx: agent_feed_tx,
        capacity: 8,
    });
    let (_, main_feed) = spawn_main_feed(TuiMainFeedConfig {
        unified_tx: main_feed_tx,
        capacity: 8,
    });
    let (_, ask_panel) = spawn_ask_panel(Count::of(8));
    let (_, chat_menu) = spawn_chat_menu(Count::of(8));
    let (_, spinner) = spawn_spinner(Count::of(8));
    let (_, controls) = spawn_controls(Count::of(8));

    TuiSubActorHandles::builder()
        .main_feed(main_feed)
        .agent_panel(agent_panel)
        .ask_panel(ask_panel)
        .overlays(
            TuiOverlayHandles::builder()
                .chat_menu(chat_menu)
                .spinner(spinner)
                .controls(controls)
                .build(),
        )
        .build()
}

/// Spawn LLM feed consumer, user message consumer, and bridge tasks.
///
/// Wires the `user_chunk` and `bg_agent` LLM output channels to the TUI
/// sub-actors via bridge tasks that forward decoded items to the main feed and
/// agent panels.
///
/// `thinking_tx` and `tool_request_tx` output receivers are intentionally
/// dropped - those feeds are not yet displayed in the TUI.
/// `raw_tx` and `parsed_tx` output receivers from the user-message consumer
/// are also intentionally dropped - no TUI consumer reads those feeds yet.
///
/// The returned `\`ConsumerHandles\`` **must** be kept alive for the duration of
/// the application. Dropping either handle closes the actor's command channel,
/// causing the actor to exit immediately and silently discard its output
/// senders, which in turn closes the bridge-task receivers and terminates the
/// bridge tasks. Callers store the handles in `\`OptionalHandles\`` and signal
/// shutdown via `\`shutdown_runtime\``.
pub fn spawn_consumer_actors(
    main_feed: TuiMainFeedPanelHandle,
    agent_panel: TuiAgentPanelHandle,
) -> ConsumerHandles {
    let (llm_outputs, bg_agent_rx, user_chunk_rx) = build_llm_feed_outputs();
    let (_, llm_feed_handle) = spawn_llm_feed_consumer(llm_outputs);
    spawn_bg_agent_bridge(bg_agent_rx, agent_panel);
    spawn_user_chunk_bridge(user_chunk_rx, main_feed);

    let (raw_tx, _) = mpsc::channel::<UserFeedMessage>(8);
    let (parsed_tx, _) = mpsc::channel::<UserFeedMessage>(8);
    let user_outputs = UserMessageOutputChannels { raw_tx, parsed_tx };
    let (_, user_msg_handle) = spawn_user_msg_consumer(user_outputs);

    ConsumerHandles {
        llm_feed: llm_feed_handle,
        user_message: user_msg_handle,
    }
}

fn build_llm_feed_outputs() -> (
    LlmFeedOutputChannels,
    mpsc::Receiver<LlmFeedMessage>,
    mpsc::Receiver<LlmFeedMessage>,
) {
    let (bg_agent_tx, bg_agent_rx) = mpsc::channel::<LlmFeedMessage>(8);
    let (thinking_tx, _) = mpsc::channel::<LlmFeedMessage>(8);
    let (user_chunk_tx, user_chunk_rx) = mpsc::channel::<LlmFeedMessage>(8);
    let (tool_request_tx, _) = mpsc::channel::<LlmFeedMessage>(8);
    let llm_outputs = LlmFeedOutputChannels::builder()
        .bg_agent_tx(bg_agent_tx)
        .thinking_tx(thinking_tx)
        .user_chunk_tx(user_chunk_tx)
        .tool_request_tx(tool_request_tx)
        .build();
    (llm_outputs, bg_agent_rx, user_chunk_rx)
}

fn spawn_bg_agent_bridge(
    mut bg_agent_rx: mpsc::Receiver<LlmFeedMessage>,
    agent_panel: TuiAgentPanelHandle,
) {
    tokio::spawn(async move {
        while let Some(msg) = bg_agent_rx.recv().await {
            if let StreamChunk::Token(text) = msg.chunk {
                agent_panel.send_agent_feed(AgentFeedOutput::StatusLine(text));
            }
        }
    });
}

fn spawn_user_chunk_bridge(
    mut user_chunk_rx: mpsc::Receiver<LlmFeedMessage>,
    main_feed: TuiMainFeedPanelHandle,
) {
    tokio::spawn(async move {
        while let Some(msg) = user_chunk_rx.recv().await {
            forward_user_chunk_to_main_feed(msg.chunk, &main_feed);
        }
    });
}

fn forward_user_chunk_to_main_feed(chunk: StreamChunk, main_feed: &TuiMainFeedPanelHandle) {
    match chunk {
        StreamChunk::Token(text) => main_feed.send_agent(AgentOutput::Token(text)),
        StreamChunk::Error(text) => main_feed.send_agent(AgentOutput::Error(text)),
        _ => {}
    }
}

/// Take the query-user receiver from `core`, logging and returning a closed receiver if already consumed.
///
/// The query channel receiver is stored as `Option<Receiver>` and must be
/// claimed exactly once by the TUI actor. A second call indicates a wiring bug:
/// it logs an error via `tracing::error!` before panicking so the failure is
/// visible in the structured log.
pub fn take_query_rx(
    core: &mut CoreRuntime,
) -> mpsc::Receiver<augur_domain::tools::builtin::query_user::QueryUserRequest> {
    match core.context.query.rx.take() {
        Some(rx) => rx,
        None => {
            tracing::error!(
                "take_query_rx: query receiver already consumed - wiring bug: \
                 take_query_rx must be called exactly once; returning closed channel"
            );
            let (_tx, rx) = mpsc::channel(1);
            rx
        }
    }
}
