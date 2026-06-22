use super::{
    ActorRuntime, AppHandles, AppJoins, AppRuntime, ChatParts, CoreRuntime, NonUiAppActors,
    OptionalHandles, OptionalJoins, PrimaryDomainHandles, PrimaryDomainJoins, PrimaryHandles,
    PrimaryJoins, PrimaryUiHandles, PrimaryUiJoins, RunRuntime, RuntimeActors, RuntimeUiChannels,
    SpawnAppFinalizeArgs, SpawnedAppActors, SpawnedDomainActors, SpawnedOptionalActors,
    SpawnedUiActors, SupervisorParts, TaskJoin, TuiBuildChannels, TuiBuildCore,
    UnpackedRuntimeActors,
};
use augur_core::actors;
use augur_domain::config::install_path::effective_repo_root;
use augur_domain::config::types::AppConfig;
use augur_domain::config::types::ProgramSettings;
use augur_domain::domain::StringNewtype;
use augur_domain::domain::channels::AGENT_FEED_CAPACITY;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::types::{AgentOutput, FeedEntry, StreamChunk};
use augur_tui::domain::tui_render::AppRenderer;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
pub struct AppRuntimeConfigRef<'a> {
    pub(crate) config: &'a AppConfig,
    pub(crate) program_settings: &'a ProgramSettings,
}

#[derive(Clone, Copy)]
struct NonUiRuntimeConfigRef<'a> {
    config: &'a AppConfig,
    program_settings: &'a ProgramSettings,
}

/// Forward `StreamChunk` items from an LLM reply channel to the agent output broadcast.
///
/// Reads chunks from `rx` until the channel closes. Converts each chunk to the
/// matching `AgentOutput` variant and sends it on `output_tx` so automated LLM
/// responses flow through the same rendering path as regular agent responses:
///
/// - `Token` → `AgentOutput::Token`
/// - `Error` → `AgentOutput::Error` (then stops)
/// - `RateLimitRetry` → `AgentOutput::Token` (notice text) + `AgentOutput::BackoffStarted`
/// - `Done` → stops the loop (signals end-of-stream)
/// - `ToolCall` / `Usage` → silently ignored (automated messages do not execute tools)
///
/// Called by the auto-message bridge in `spawn_app_runtime` for each automated
/// message so the LLM response is not silently discarded.
pub async fn forward_reply_to_broadcast(
    mut rx: mpsc::Receiver<StreamChunk>,
    output_tx: tokio::sync::broadcast::Sender<AgentOutput>,
) {
    while let Some(chunk) = rx.recv().await {
        if !forward_stream_chunk(chunk, &output_tx) {
            break;
        }
    }
}

fn forward_stream_chunk(
    chunk: StreamChunk,
    output_tx: &tokio::sync::broadcast::Sender<AgentOutput>,
) -> bool {
    if let StreamChunk::Done = chunk {
        return false;
    }
    if let StreamChunk::Error(error) = chunk {
        let _ = output_tx.send(AgentOutput::Error(error));
        return false;
    }
    if let StreamChunk::RateLimitRetry(secs) = chunk {
        send_rate_limit_retry_notice(output_tx, secs);
        return true;
    }
    if let StreamChunk::Token(token) = chunk {
        let _ = output_tx.send(AgentOutput::Token(token));
    }
    true
}

fn send_rate_limit_retry_notice(
    output_tx: &tokio::sync::broadcast::Sender<AgentOutput>,
    secs: augur_domain::domain::newtypes::WaitSecs,
) {
    let notice = format!("[rate limit - waiting {}s...]\n", secs.inner());
    let _ = output_tx.send(AgentOutput::Token(augur_domain::domain::OutputText::new(
        notice,
    )));
    let _ = output_tx.send(AgentOutput::BackoffStarted(secs));
}

/// Wrap a `(join, handle)` pair into an [`ActorRuntime`].
///
/// Convenience constructor used throughout the wiring layer to convert the
/// two-tuple returned by actor `spawn` functions into the structured
/// [`ActorRuntime`] type.
pub fn actor_runtime<H>((join, handle): (TaskJoin, H)) -> super::ActorRuntime<H> {
    super::ActorRuntime { join, handle }
}

/// Spawn the deterministic orchestrator actor at `repo_root` and return its runtime.
///
/// Passes `repo_root` and `feed_tx` to the orchestrator's spawn function and
/// wraps the result in an [`ActorRuntime`]. `feed_tx` is the channel used to
/// deliver `FeedEntry` items to TUI consumers.
pub fn spawn_deterministic_orchestrator_runtime(
    repo_root: std::path::PathBuf,
    feed_tx: mpsc::Sender<FeedEntry>,
) -> ActorRuntime<actors::DeterministicOrchestratorHandle> {
    let dispatch_runtime = Arc::new(CopilotDeterministicDispatchRuntime {});
    actor_runtime(
        actors::deterministic_orchestrator::deterministic_orchestrator_actor::spawn_with_join_and_feed_and_runtime(
            repo_root,
            feed_tx,
            dispatch_runtime,
        ),
    )
}

/// Spawn the deterministic orchestrator rooted at the process working directory.
///
/// Resolves the repo root via [`std::env::current_dir`] (falling back to `"."`)
/// and delegates to [`spawn_deterministic_orchestrator_runtime`].
pub fn spawn_root_deterministic_orchestrator_runtime(
    feed_tx: mpsc::Sender<FeedEntry>,
) -> ActorRuntime<actors::DeterministicOrchestratorHandle> {
    spawn_deterministic_orchestrator_runtime(current_repo_root(), feed_tx)
}

fn current_repo_root() -> std::path::PathBuf {
    effective_repo_root()
}

#[derive(Debug, Default)]
struct CopilotDeterministicDispatchRuntime {}

impl augur_core::actors::deterministic_orchestrator::background_dispatch::BackgroundAgentRuntime
    for CopilotDeterministicDispatchRuntime
{
    fn dispatch(
        &self,
        launch: augur_core::actors::deterministic_orchestrator::background_dispatch::BackgroundAgentLaunch,
    ) -> Result<
        augur_core::actors::deterministic_orchestrator::background_dispatch::BackgroundRuntimeTicket,
        augur_core::actors::deterministic_orchestrator::background_dispatch::DispatchError,
    >{
        let (feed_tx, feed_rx) = mpsc::channel(AGENT_FEED_CAPACITY.inner());
        let (signal_tx, signal_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(augur_provider_copilot_sdk::actors::copilot::background_agent::run_background_agent(
            augur_provider_copilot_sdk::actors::copilot::background_agent::BackgroundAgentArgs::builder()
                .config(
                    augur_provider_copilot_sdk::actors::copilot::background_agent::BackgroundAgentConfig::builder()
                        .agent(launch.agent)
                        .feed_id(launch.feed_id)
                        .prompt(launch.prompt)
                        .maybe_model(launch.model)
                        .build(),
                )
                .feed_tx(feed_tx)
                .signal_tx(signal_tx)
                .classifier(Arc::new(
                    augur_provider_copilot_sdk::actors::copilot::event_classifier::CopilotEventClassifier,
                ))
                .build(),
        ));
        Ok(
            augur_core::actors::deterministic_orchestrator::background_dispatch::BackgroundRuntimeTicket::new(
                task,
                feed_rx,
                Some(signal_rx),
            ),
        )
    }
}

/// Wire orchestrator auto-messages → LLM for hands-free pipeline continuation.
///
/// Spawns a task that bridges automated messages from the deterministic
/// orchestrator to the LLM actor, forwarding each reply back to the agent
/// output broadcast so the TUI and other subscribers see the response.
pub(super) fn wire_auto_message_bridge(core: &CoreRuntime, domain: &SpawnedDomainActors) {
    let mut auto_msg_rx = domain
        .deterministic_orchestrator
        .handle
        .subscribe_automated_messages();
    let llm = core.handles.services.llm.clone();
    let session = domain.session.handle.clone();
    let agent_output_tx = domain.agent.handle.clone_output_tx();
    tokio::spawn(async move {
        loop {
            match auto_msg_rx.recv().await {
                Ok(msg) => {
                    let endpoint = session.active_endpoint();
                    let reply_rx = llm.send_automated(msg.0, endpoint);
                    let fwd_tx = agent_output_tx.clone();
                    tokio::spawn(forward_reply_to_broadcast(reply_rx, fwd_tx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        skipped = n,
                        "auto-message bridge lagged; {n} automated messages dropped"
                    );
                    continue;
                }
            }
        }
    });
}

/// Spawn all application actors and assemble the full [`RunRuntime`].
///
/// Creates the agent-feed channel, spawns domain, supervisor, chat, and
/// planning actors, wires the auto-message bridge from the deterministic
/// orchestrator to the LLM, then finalises by spawning the TUI actor and
/// collecting all joins and handles into a [`RunRuntime`].
///
/// `config` drives actor configuration, `renderer` is handed to the TUI, and
/// `core` supplies infrastructure handles (LLM, tools, logger, etc.).
pub async fn spawn_app_runtime(
    runtime_config: AppRuntimeConfigRef<'_>,
    renderer: AppRenderer,
    mut core: CoreRuntime,
) -> RunRuntime {
    use augur_domain::domain::channels::AGENT_FEED_CAPACITY;

    let (feed_tx, feed_rx) = mpsc::channel::<FeedEntry>(*AGENT_FEED_CAPACITY);
    let app_actors = spawn_non_ui_app_actors(
        NonUiRuntimeConfigRef {
            config: runtime_config.config,
            program_settings: runtime_config.program_settings,
        },
        &mut core,
        feed_tx.clone(),
    )
    .await;
    let NonUiAppActors {
        domain,
        supervisor,
        chat,
        planning,
    } = app_actors;
    let sub_actors = super::spawn_tui_sub_actors();
    let consumer_handles =
        super::spawn_consumer_actors(sub_actors.main_feed.clone(), sub_actors.agent_panel.clone());

    wire_auto_message_bridge(&core, &domain);
    finalize_spawn_app_runtime(SpawnAppFinalizeArgs {
        core,
        config: runtime_config.config,
        renderer,
        actors: RuntimeActors {
            domain,
            planning,
            chat,
            supervisor,
            consumer_handles,
        },
        ui_channels: RuntimeUiChannels {
            feed_tx,
            feed_rx,
            sub_actors,
        },
    })
}

fn finalize_spawn_app_runtime(args: SpawnAppFinalizeArgs<'_>) -> RunRuntime {
    let SpawnAppFinalizeArgs {
        mut core,
        config,
        renderer,
        actors,
        ui_channels,
    } = args;
    let unpacked = unpack_runtime_actors(actors);
    let tui_deps = super::build_spawned_tui_deps(
        TuiBuildCore {
            config,
            renderer,
            domain: &unpacked.domain,
            planning: &unpacked.planning,
            chat_provider: unpacked.chat.provider.clone(),
        },
        TuiBuildChannels {
            output_rx: unpacked.chat.output_rx,
            supervisor_rx: unpacked.supervisor.rx,
            feed_tx: ui_channels.feed_tx,
            feed_rx: ui_channels.feed_rx,
        },
    );
    let tui = super::spawn_tui_actor(
        &mut core,
        super::build_tui_deps(tui_deps.startup, tui_deps.services, tui_deps.channels),
        ui_channels.sub_actors,
    );
    build_run_runtime(
        core,
        SpawnedAppActors {
            domain: unpacked.domain,
            planning: unpacked.planning,
            ui: SpawnedUiActors { tui },
            optional: SpawnedOptionalActors {
                executor_join: unpacked.supervisor.join,
                supervisor_handle: unpacked.supervisor.handle,
                chat_join: unpacked.chat.join,
                chat_provider: unpacked.chat.provider,
                consumer_handles: unpacked.consumer_handles,
            },
        },
    )
}

fn unpack_runtime_actors(actors: RuntimeActors) -> UnpackedRuntimeActors {
    UnpackedRuntimeActors {
        domain: actors.domain,
        planning: actors.planning,
        chat: ChatParts {
            provider: actors.chat.provider,
            output_rx: actors.chat.output_rx,
            join: actors.chat.join,
        },
        supervisor: SupervisorParts {
            rx: actors.supervisor.rx,
            join: actors.supervisor.join,
            handle: actors.supervisor.handle,
        },
        consumer_handles: actors.consumer_handles,
    }
}

async fn spawn_non_ui_app_actors(
    runtime_config: NonUiRuntimeConfigRef<'_>,
    core: &mut CoreRuntime,
    feed_tx: mpsc::Sender<FeedEntry>,
) -> NonUiAppActors {
    let domain = super::spawn_domain_actors(
        super::DomainRuntimeConfigRef {
            config: runtime_config.config,
            program_settings: runtime_config.program_settings,
        },
        core,
        feed_tx.clone(),
    )
    .await;
    let supervisor = super::spawn_supervisor_runtime(runtime_config.config).await;
    let chat = super::spawn_chat_runtime(
        runtime_config.config,
        core,
        super::ChatRuntimeInput {
            agent_handle: domain.agent.handle.clone(),
            session_handle: domain.session.handle.clone(),
            agent_feed_tx: feed_tx,
        },
    )
    .await;
    let planning = super::spawn_planning_actors();
    NonUiAppActors {
        domain,
        supervisor,
        chat,
        planning,
    }
}

/// Assemble a [`RunRuntime`] from a complete set of spawned actors.
///
/// Distributes join handles into `\`AppJoins\`` and actor handles into
/// `\`AppHandles\``, nesting them under the primary / optional / domain / UI
/// hierarchy expected by the shutdown and access paths.
pub fn build_run_runtime(core: CoreRuntime, actors: SpawnedAppActors) -> RunRuntime {
    RunRuntime {
        core,
        app: AppRuntime {
            joins: AppJoins {
                primary: PrimaryJoins {
                    domain: PrimaryDomainJoins {
                        agent: actors.domain.agent.join,
                        session: actors.domain.session.join,
                        ask_agent: actors.domain.ask.join,
                        deterministic_orchestrator: actors.domain.deterministic_orchestrator.join,
                        file_scanner: actors.planning.file_scanner.join,
                    },
                    ui: PrimaryUiJoins {
                        tui: actors.ui.tui.join,
                    },
                },
                optional: OptionalJoins {
                    ask_tool: actors.domain.ask.tool_join,
                    copilot: actors.optional.chat_join,
                    executor: actors.optional.executor_join,
                },
            },
            handles: AppHandles {
                primary: PrimaryHandles {
                    domain: PrimaryDomainHandles {
                        agent: actors.domain.agent.handle,
                        session: actors.domain.session.handle,
                        file_scanner: actors.planning.file_scanner.handle,
                        guided_plan: actors.planning.guided_plan,
                        deterministic_orchestrator: actors.domain.deterministic_orchestrator.handle,
                    },
                    ui: PrimaryUiHandles {
                        tui: actors.ui.tui.handle,
                    },
                },
                optional: OptionalHandles {
                    ask_shutdown: actors.domain.ask.handle,
                    chat_provider: actors.optional.chat_provider,
                    supervisor: actors.optional.supervisor_handle,
                    consumers: actors.optional.consumer_handles,
                },
            },
        },
    }
}
