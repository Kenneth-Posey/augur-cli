//! TUI actor: owns the Ratatui terminal, event loop, and AppState.

mod guided_plan;
mod runtime;
use super::tui_actor_ops as actor_ops;

use super::assistant::key_dispatch::dispatch_chat_key;
use super::assistant::output_buf::drain_channel_to_buf;
use super::assistant::session_restore::apply_restored_session;
use super::handle::ShutdownSignal;
use super::handle::TuiHandle;
use crate::domain::tui_render::AppRenderer;
use augur_core::actors::command::handle::CommandHandle;
use augur_core::actors::file_scanner::FileScannerHandle;
use augur_core::actors::session::handle::SessionHandle;
use augur_core::actors::token_tracker::TokenTrackerHandle;
use augur_core::domain::deterministic_orchestrator::DeterministicOrchestratorEvent;
use augur_domain::config::types::AppConfig;
use augur_domain::domain::traits::ChatProvider;
use augur_domain::domain::types::{AgentOutput, FeedEntry, SupervisorEvent};
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::persistence::types::SessionSummary;
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};

pub use runtime::layout::TuiOverlayHandles;
pub use runtime::layout::TuiSubActorHandles;

use guided_plan::{apply_guided_plan_actions, handle_guided_plan_event};
use runtime::run;
use runtime::{
    configure_terminal_startup, handle_mouse_event, maybe_finish_guided_plan_compaction,
};

/// Startup data for the TUI actor: session history and shared handles.
///
/// Extracted from `TuiSpawnArgs` to keep that struct within the 5-field limit.
/// Groups the values needed only at startup or by background tasks (session list,
/// persistence, token-tracker, and app config for status bar initialization).
#[derive(bon::Builder)]
pub struct TuiStartupData {
    /// Session summaries loaded at startup; non-empty triggers the picker screen.
    pub session_summaries: Vec<SessionSummary>,
    /// Handle to the persistence layer for session restore.
    pub persistence: PersistenceHandle,
    /// Handle to the token-tracker actor for periodic snapshot ticks.
    pub token_tracker: TokenTrackerHandle,
    /// Application configuration (endpoints, agent settings). Used at startup
    /// to initialize the status bar model label.
    pub config: AppConfig,
    /// Injected render function owned by the higher TUI shell layer.
    pub renderer: AppRenderer,
}

/// Bundled tool accessory handles for the TUI actor.
///
/// Extracted from `TuiServiceHandles` to keep that struct within the 5-field
/// limit. Groups the command registry, file scanner, guided plan, ask-panel,
/// and logger handles used in key dispatch and the event loop.
///
/// Consumers: `TuiServiceHandles.tools`, `TuiToolHandles<'a>` borrows, `wiring.rs`.
#[derive(bon::Builder)]
pub struct TuiServiceTools {
    /// Handle to the command registry for slash commands and hint lines.
    pub command: CommandHandle,
    /// Handle to the file scanner actor for `@` path autocompletion.
    pub file_scanner: FileScannerHandle,
    /// Handle to the guided plan actor for file-driven plan execution.
    pub guided_plan: augur_core::actors::guided_plan::GuidedPlanHandle,
    /// Handle to the ask-panel agent actor for side-channel LLM conversations.
    pub ask: augur_core::actors::ask::AskHandle,
    /// Logger handle for recording TUI events to the session JSONL log.
    pub logger: augur_core::actors::LoggerHandle,
}

/// Bundled workflow handles for TUI commands that trigger actor-side actions.
///
/// Extracted from `TuiHandles` to keep that struct within the 5-field limit.
/// Groups the orchestrator and catalog-manager handles, which are only used by
/// slash-command dispatch and session restore flows.
#[derive(bon::Builder)]
pub struct TuiWorkHandles {
    /// Handle to the deterministic orchestrator for `/run-pipeline` dispatch.
    pub orchestrator: augur_core::actors::DeterministicOrchestratorHandle,
    /// Handle to the catalog manager actor for `/generate-catalog` dispatch.
    pub catalog_manager: augur_core::actors::catalog_manager::CatalogManagerHandle,
}

/// Bundled service handles for the TUI actor: chat provider, session, tools, and pipeline.
///
/// Extracted from `TuiSpawnArgs` to keep that struct at 3 fields. Tool
/// accessory handles are further grouped in `TuiServiceTools` to keep this
/// struct within the 5-field limit.
///
/// Consumers: `TuiSpawnArgs.providers`, `wiring.rs`.
#[derive(bon::Builder)]
pub struct TuiServiceHandles {
    /// Chat provider - either `AgentHandle` or `CopilotChatHandle`, type-erased.
    ///
    /// `wiring.rs` wraps the chosen concrete type as `Arc<dyn ChatProvider>`.
    pub agent: Arc<dyn ChatProvider>,
    /// Handle to the session actor for reading the active endpoint.
    pub session: SessionHandle,
    /// Tool accessory handles: command, file scanner, guided plan, and ask panel.
    pub tools: TuiServiceTools,
    /// Handle to the deterministic orchestrator for `/run-pipeline` dispatch.
    pub orchestrator: augur_core::actors::DeterministicOrchestratorHandle,
    /// Handle to the catalog manager actor for `/generate-catalog` dispatch.
    pub catalog_manager: augur_core::actors::catalog_manager::CatalogManagerHandle,
}

/// Arguments to `TuiActor::spawn`. Groups all actor dependencies into one struct.
#[derive(bon::Builder)]
pub struct TuiSpawnArgs {
    /// Bundled service handles: agent, session, command registry, file scanner.
    pub providers: TuiServiceHandles,
    /// Bundled input channel receivers for the TUI event loop.
    pub channels: TuiInputChannels,
    /// Startup data: session summaries, persistence, project settings, config.
    pub startup: TuiStartupData,
    /// Handles to the five TUI sub-actors (agent panel, main feed, chat menu,
    /// spinner, dynamic controls) used for per-frame watch-channel snapshot reads.
    pub sub_actors: TuiSubActorHandles,
}

/// Bundled input channel receivers for the TUI actor.
///
/// Groups the agent output broadcast receiver, the query request mpsc receiver,
/// and the optional supervisor event broadcast receiver so `TuiSpawnArgs` stays
/// within the 5-field limit and `select_next_event` receives all channels in a
/// single argument.
#[derive(bon::Builder)]
pub struct TuiInputChannels {
    /// Broadcast receiver for agent output tokens and status events.
    pub output_rx: broadcast::Receiver<AgentOutput>,
    /// Mpsc receiver for query requests from the `query_user` tool.
    pub query_rx: mpsc::Receiver<QueryUserRequest>,
    /// Optional broadcast receiver for supervisor plan events.
    ///
    /// `None` when the supervisor actor has not been spawned (e.g., the
    /// `copilot-executor` feature is not enabled or no plan is active).
    pub supervisor_rx: Option<broadcast::Receiver<SupervisorEvent>>,
}

/// Bundles the background channel receivers for the TUI event loop.
///
/// Groups the supervisor event, agent feed, and orchestrator event receivers -
/// all "background output" sources - so `TuiChannelStreams` stays within the
/// 5-field limit.
#[derive(bon::Builder)]
struct TuiBackgroundChannels<'a> {
    supervisor_rx: Option<&'a mut broadcast::Receiver<SupervisorEvent>>,
    agent_feed_rx: &'a mut mpsc::Receiver<FeedEntry>,
    orchestrator_event_rx: &'a mut broadcast::Receiver<DeterministicOrchestratorEvent>,
}

/// Bundles the channel receivers the TUI event loop reads from.
///
/// Extracted from `TuiStreams` to keep that struct within the 5-field limit.
/// Groups the main agent output, ask-panel output, query, and guided-plan
/// broadcast/mpsc receivers. Background sources (supervisor and agent feed)
/// are grouped in `TuiBackgroundChannels`.
#[derive(bon::Builder)]
struct TuiChannelStreams<'a> {
    output_rx: &'a mut broadcast::Receiver<AgentOutput>,
    ask_output_rx: &'a mut broadcast::Receiver<AgentOutput>,
    query_rx: &'a mut mpsc::Receiver<QueryUserRequest>,
    guided_plan_rx: &'a mut broadcast::Receiver<augur_domain::domain::guided_plan::GuidedPlanEvent>,
    background: TuiBackgroundChannels<'a>,
}

/// Carries references needed to poll the periodic token snapshot ticker.
///
/// Extracted from `TuiStreams` so the snapshot arm in `select_next_event`
/// can borrow both the ticker and the handle simultaneously without
/// violating the borrow checker.
struct TuiSnapshotState<'a> {
    /// The 1-second interval ticker driving snapshot polls.
    ticker: &'a mut tokio::time::Interval,
    /// Handle to the token-tracker actor for requesting totals.
    token_tracker: &'a TokenTrackerHandle,
}

/// Bundles the live event streams the TUI actor reads from each iteration.
///
/// Also carries `char_buf` so `select_next_event` stays within the 3-parameter
/// limit while retaining full access to the animation buffer.
#[derive(bon::Builder)]
struct TuiStreams<'a> {
    event_stream: &'a mut crossterm::event::EventStream,
    channels: TuiChannelStreams<'a>,
    ticker: &'a mut tokio::time::Interval,
    char_buf: &'a mut augur_domain::domain::string_newtypes::OutputText,
    snapshot: TuiSnapshotState<'a>,
}

/// Bundles the UI tool references needed by key dispatch helpers.
///
/// Extracted from `TuiHandles` to keep that struct within the 5-field limit.
/// Groups the command registry, file scanner, guided plan, ask-panel, and logger
/// handles used in key dispatch and command handling.
#[derive(bon::Builder)]
pub(crate) struct TuiToolHandles<'a> {
    pub(crate) command: &'a CommandHandle,
    pub(crate) file_scanner: &'a FileScannerHandle,
    pub(crate) guided_plan: &'a augur_core::actors::guided_plan::GuidedPlanHandle,
    pub(crate) ask: &'a augur_core::actors::ask::AskHandle,
    pub(crate) logger: &'a augur_core::actors::LoggerHandle,
}

/// Bundles immutable references to the actor handles needed for dispatching.
///
/// Passed to `select_next_event`, `handle_submit`, and `restore_session` so
/// no individual function exceeds the 3-parameter limit. `tools` groups the
/// UI-layer accessory handles (command registry, file scanner, guided plan).
#[derive(bon::Builder)]
pub(crate) struct TuiHandles<'a> {
    pub(crate) agent: &'a dyn ChatProvider,
    pub(crate) session: &'a SessionHandle,
    pub(crate) persistence: &'a PersistenceHandle,
    pub(crate) tools: TuiToolHandles<'a>,
    pub(crate) work: TuiWorkHandles,
}

/// Describes what the TUI main loop should do after processing one event.
///
/// Returned by `select_next_event` to decide whether the terminal is re-rendered.
/// `NoOp` prevents wasteful renders when no visible state changed - the primary
/// case being free-motion mouse events from the `?1003h` all-motion protocol
/// enabled by `EnableMouseCapture`, which flood the loop at idle.
enum EventOutcome {
    /// The user or system has requested the TUI to exit.
    Quit,
    /// Visible state changed; re-render the terminal.
    Redraw,
    /// No visible state changed; skip the render for this iteration.
    NoOp,
}

/// Number of characters drained from the animation buffer per 20ms ticker tick.
///
/// At 20ms/tick (50 ticks/sec), 6 chars/tick yields ~300 chars/sec display rate.
/// This produces a smooth continuous stream regardless of how tokens arrive from
/// the API. A burst of tokens fills the buffer; the ticker drains it steadily.
const CHARS_PER_TICK: usize = 6;

/// Animation ticker interval in milliseconds.
///
/// Sets the rate at which the TUI redraws and drains `CHARS_PER_TICK` characters
/// from the token animation buffer. At 20ms (50 Hz) the display is smooth without
/// excessive CPU usage. Paired with `CHARS_PER_TICK` to control output pacing.
const TICKER_INTERVAL_MS: u64 = 20;
/// Static title string written to the terminal window title bar on startup.
///
/// Passed to `crossterm::terminal::SetTitle` during `configure_terminal_startup`.
/// Changing this value updates the title shown in the OS task bar or tab strip.
const TERMINAL_TITLE: &str = "augur-cli";

/// Spawn the TUI actor task.
///
/// The actor task owns the terminal and `AppState`. Sends `true` on the
/// shutdown watch channel when the event loop exits so `main` can join cleanly.
/// Accepts an externally created agent feed channel pair (`feed_tx`, `feed_rx`)
/// so the channel is created in `wiring.rs` and shared with the Copilot actor.
/// The handle stores `feed_tx`; the actor task receives on `feed_rx`.
#[tracing::instrument(skip_all, level = "info")]
pub fn spawn(
    args: TuiSpawnArgs,
    feed_tx: mpsc::Sender<FeedEntry>,
    feed_rx: mpsc::Receiver<FeedEntry>,
) -> (tokio::task::JoinHandle<()>, TuiHandle) {
    let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
    let handle = TuiHandle::new(shutdown_rx, feed_tx);
    let join = actor_ops::spawn_run(args, shutdown_tx, feed_rx);
    (join, handle)
}

/// Spawn the asynchronous TUI runtime loop on Tokio.
pub(super) fn spawn_runtime_task(
    args: TuiSpawnArgs,
    shutdown_tx: watch::Sender<ShutdownSignal>,
    feed_rx: mpsc::Receiver<FeedEntry>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(args, shutdown_tx, feed_rx))
}
