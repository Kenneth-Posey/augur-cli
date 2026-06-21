//! Runtime helpers for the TUI actor event loop.

mod events;
pub mod layout;
mod state;
mod terminal;

use super::{TuiHandles, TuiStreams, TERMINAL_TITLE, TICKER_INTERVAL_MS};
use crate::actors::tui::assistant::output_buf::drain_channel_to_buf;
use crate::actors::tui::handle::ShutdownSignal;
use crate::domain::tui_state::AppState;
use augur_core::domain::deterministic_orchestrator::DeterministicOrchestratorEvent;
use augur_domain::domain::string_newtypes::OutputText;
use augur_domain::domain::types::{AgentOutput, FeedEntry, SupervisorEvent};
use augur_domain::tools::builtin::query_user::QueryUserRequest;
use tokio::sync::{broadcast, mpsc, watch};

use augur_core::actors::token_tracker::TokenTrackerHandle;
use events::select_next_event;
use layout::{collect_render_snapshot, render_layout, TuiSubActorHandles};
use state::build_initial_state;

/// Re-exported mouse-event handler used by actor-level tests.
pub(super) use terminal::handle_mouse_event;

/// Snapshot ticker interval in milliseconds.
///
/// Drives the periodic token-snapshot poll that refreshes the status bar
/// with accumulated lifetime token totals. At 1000ms (1 Hz) the display
/// stays current without polling the token-tracker actor unnecessarily.
const SNAPSHOT_INTERVAL_MS: u64 = 1000;
use terminal::shutdown_runtime;

/// Enable terminal features needed by the interactive TUI runtime.
pub(super) fn configure_terminal_startup<W: std::io::Write>(writer: &mut W) -> std::io::Result<()> {
    crossterm::execute!(
        writer,
        crossterm::terminal::SetTitle(TERMINAL_TITLE),
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste,
    )
}

/// Notify guided-plan tools when a pending compaction has completed.
pub(super) fn maybe_finish_guided_plan_compaction(
    state: &mut AppState,
    is_compaction_done: Option<()>,
    handles: &TuiHandles<'_>,
) {
    if is_compaction_done.is_some()
        && matches!(
            &state.interaction.mode,
            crate::domain::tui_state::ConversationMode::GuidedPlan(ui)
                if ui.guided_awaiting_compact.into()
        )
    {
        handles.tools.guided_plan.compaction_done();
        state.clear_guided_plan_compact_flag();
    }
}

/// Run the TUI actor event loop until quit or terminal shutdown.
pub(super) async fn run(
    args: super::TuiSpawnArgs,
    shutdown_tx: watch::Sender<ShutdownSignal>,
    agent_feed_rx: mpsc::Receiver<FeedEntry>,
) {
    let mut terminal = ratatui::init();
    let mut stdout = std::io::stdout();
    let _ = configure_terminal_startup(&mut stdout);
    let super::TuiSpawnArgs {
        providers,
        channels,
        startup,
        sub_actors,
    } = args;
    let mut state = build_initial_state(&providers, &startup);
    let orchestrator_event_rx = providers.orchestrator.subscribe();
    let background = RuntimeBackgroundInput {
        agent_feed_rx,
        orchestrator_event_rx,
    };
    let mut runtime = RuntimeLoop::new(
        RuntimeLoopArgs::builder()
            .channels(channels)
            .tools(&providers.tools)
            .background(background)
            .token_tracker(startup.token_tracker.clone())
            .build(),
    );
    let handles = build_handles(&providers, &startup.persistence);
    let renderer = startup.renderer;

    // Initialize output_area with terminal dimensions BEFORE the event loop starts.
    // This ensures mouse scroll events arriving before the first render are correctly
    // classified instead of being ignored due to zero-sized panel_areas.
    initialize_panel_areas(&mut terminal, &mut state);

    let mut runtime_ctx = RuntimeContext::new(
        RuntimeContextArgs::builder()
            .terminal(&mut terminal)
            .sub_actors(sub_actors)
            .handles(handles)
            .renderer(renderer)
            .build(),
    );

    draw_state(&mut state, &mut runtime_ctx);
    run_loop(&mut state, &mut runtime, &mut runtime_ctx).await;
    shutdown_runtime(shutdown_tx);
}

struct RuntimeLoop {
    ui: RuntimeUi,
    channels: RuntimeChannels,
    background: RuntimeBackgroundChannels,
    token_tracker: TokenTrackerHandle,
}

struct RuntimeUi {
    event_stream: crossterm::event::EventStream,
    ticker: tokio::time::Interval,
    snapshot_ticker: tokio::time::Interval,
    char_buf: OutputText,
}

struct RuntimeChannels {
    output_rx: broadcast::Receiver<AgentOutput>,
    query_rx: mpsc::Receiver<QueryUserRequest>,
    guided_plan_rx: broadcast::Receiver<augur_domain::domain::guided_plan::GuidedPlanEvent>,
    ask_output_rx: broadcast::Receiver<AgentOutput>,
}

struct RuntimeBackgroundChannels {
    supervisor_rx: Option<broadcast::Receiver<SupervisorEvent>>,
    agent_feed_rx: mpsc::Receiver<FeedEntry>,
    orchestrator_event_rx: broadcast::Receiver<DeterministicOrchestratorEvent>,
}

/// Bundles the background channel receivers passed into `RuntimeLoop::new`.
///
/// Extracted so `RuntimeLoop::new` stays within the 3-parameter limit while
/// accepting both the agent-feed and orchestrator-event receivers, which are
/// created outside the function.
struct RuntimeBackgroundInput {
    agent_feed_rx: mpsc::Receiver<FeedEntry>,
    orchestrator_event_rx: broadcast::Receiver<DeterministicOrchestratorEvent>,
}

/// Arguments for constructing a [`RuntimeLoop`].
///
/// Bundles the four construction inputs so `RuntimeLoop::new` stays within
/// the three-parameter limit.
#[derive(bon::Builder)]
struct RuntimeLoopArgs<'a> {
    /// Input channels from the TUI actor spawn args.
    channels: super::TuiInputChannels,
    /// Service tools for subscribing to guided-plan and ask-output broadcasts.
    tools: &'a super::TuiServiceTools,
    /// Pre-constructed background channel receivers.
    background: RuntimeBackgroundInput,
    /// Token tracker handle for periodic snapshot polling.
    token_tracker: TokenTrackerHandle,
}

impl RuntimeLoop {
    fn new(args: RuntimeLoopArgs<'_>) -> Self {
        let RuntimeLoopArgs {
            channels,
            tools,
            background,
            token_tracker,
        } = args;
        let mut ticker =
            tokio::time::interval(std::time::Duration::from_millis(TICKER_INTERVAL_MS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut snapshot_ticker =
            tokio::time::interval(std::time::Duration::from_millis(SNAPSHOT_INTERVAL_MS));
        snapshot_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        Self {
            ui: RuntimeUi {
                event_stream: crossterm::event::EventStream::new(),
                ticker,
                snapshot_ticker,
                char_buf: OutputText::from(""),
            },
            channels: RuntimeChannels {
                output_rx: channels.output_rx,
                query_rx: channels.query_rx,
                guided_plan_rx: tools.guided_plan.subscribe(),
                ask_output_rx: tools.ask.subscribe_output(),
            },
            background: RuntimeBackgroundChannels {
                supervisor_rx: channels.supervisor_rx,
                agent_feed_rx: background.agent_feed_rx,
                orchestrator_event_rx: background.orchestrator_event_rx,
            },
            token_tracker,
        }
    }

    fn streams(&mut self) -> TuiStreams<'_> {
        super::TuiStreams::builder()
            .event_stream(&mut self.ui.event_stream)
            .channels(
                super::TuiChannelStreams::builder()
                    .output_rx(&mut self.channels.output_rx)
                    .ask_output_rx(&mut self.channels.ask_output_rx)
                    .query_rx(&mut self.channels.query_rx)
                    .guided_plan_rx(&mut self.channels.guided_plan_rx)
                    .background(
                        super::TuiBackgroundChannels::builder()
                            .maybe_supervisor_rx(self.background.supervisor_rx.as_mut())
                            .agent_feed_rx(&mut self.background.agent_feed_rx)
                            .orchestrator_event_rx(&mut self.background.orchestrator_event_rx)
                            .build(),
                    )
                    .build(),
            )
            .ticker(&mut self.ui.ticker)
            .char_buf(&mut self.ui.char_buf)
            .snapshot(super::TuiSnapshotState {
                ticker: &mut self.ui.snapshot_ticker,
                token_tracker: &self.token_tracker,
            })
            .build()
    }

    fn drain_output(&mut self, state: &mut AppState) -> bool {
        drain_channel_to_buf(state, &mut self.channels.output_rx, &mut self.ui.char_buf).is_some()
    }
}

/// Arguments for constructing a [`RuntimeContext`].
///
/// Bundles the four construction inputs so `RuntimeContext::new` stays within
/// the three-parameter limit.
#[derive(bon::Builder)]
struct RuntimeContextArgs<'t, 'h> {
    /// Mutable reference to the ratatui terminal for rendering.
    terminal: &'t mut ratatui::DefaultTerminal,
    /// Handles to the TUI sub-actor tasks.
    sub_actors: TuiSubActorHandles,
    /// Shared handles into service actors used during the event loop.
    handles: TuiHandles<'h>,
    /// Renderer configuration for the app layout.
    renderer: crate::domain::tui_render::AppRenderer,
}

struct RuntimeContext<'terminal, 'handles> {
    terminal: &'terminal mut ratatui::DefaultTerminal,
    sub_actors: TuiSubActorHandles,
    handles: TuiHandles<'handles>,
    renderer: crate::domain::tui_render::AppRenderer,
}

impl<'terminal, 'handles> RuntimeContext<'terminal, 'handles> {
    fn new(args: RuntimeContextArgs<'terminal, 'handles>) -> Self {
        let RuntimeContextArgs {
            terminal,
            sub_actors,
            handles,
            renderer,
        } = args;
        Self {
            terminal,
            sub_actors,
            handles,
            renderer,
        }
    }
}

async fn run_loop(
    state: &mut AppState,
    runtime: &mut RuntimeLoop,
    runtime_ctx: &mut RuntimeContext<'_, '_>,
) {
    loop {
        let outcome = select_next_event(state, runtime.streams(), &runtime_ctx.handles).await;
        if matches!(&outcome, super::EventOutcome::Quit) {
            break;
        }
        let drained_output = runtime.drain_output(state);
        let outcome_requests_redraw = matches!(&outcome, super::EventOutcome::Redraw);
        let should_redraw = outcome_requests_redraw || drained_output;
        if should_redraw {
            draw_state(state, runtime_ctx);
        }
    }
}

fn build_handles<'a>(
    providers: &'a super::TuiServiceHandles,
    persistence: &'a augur_domain::persistence::handle::PersistenceHandle,
) -> TuiHandles<'a> {
    super::TuiHandles::builder()
        .agent(providers.agent.as_ref())
        .session(&providers.session)
        .persistence(persistence)
        .tools(
            super::TuiToolHandles::builder()
                .command(&providers.tools.command)
                .file_scanner(&providers.tools.file_scanner)
                .guided_plan(&providers.tools.guided_plan)
                .ask(&providers.tools.ask)
                .logger(&providers.tools.logger)
                .build(),
        )
        .work(
            super::TuiWorkHandles::builder()
                .orchestrator(providers.orchestrator.clone())
                .catalog_manager(providers.catalog_manager.clone())
                .build(),
        )
        .build()
}

fn draw_state(state: &mut AppState, runtime_ctx: &mut RuntimeContext<'_, '_>) {
    state.agent.endpoint_name = runtime_ctx.handles.session.active_endpoint();
    let snapshot = collect_render_snapshot(&runtime_ctx.sub_actors, runtime_ctx.renderer);
    let display = crate::domain::tui_display_state::TuiDisplayState::project_from(state);
    let _ = runtime_ctx
        .terminal
        .draw(|frame| render_layout(frame, &snapshot, &display));
    // Render writes panel bounds and width-adjusted scroll via interior mutability on the
    // display snapshot. Copy those fields back so mouse routing uses current panel areas.
    state.output.panel_areas = display.output.panel_areas.clone();
    state
        .output
        .scroll_offset
        .set(display.output.scroll_offset.get());
    state
        .output
        .last_render_width
        .set(display.output.last_render_width.get());
}

/// Initialize panel areas with terminal dimensions before the event loop starts.
///
/// This ensures that mouse scroll events arriving before the first render are
/// correctly classified instead of being ignored due to zero-sized panel_areas.
/// The output_area starts as Rect::default() (zero dimensions), which causes
/// the mouse classifier to ignore all scroll events until the first render occurs.
/// By proactively setting output_area to the terminal size, we guarantee that
/// scroll events are handled from the start.
fn initialize_panel_areas(terminal: &mut ratatui::DefaultTerminal, state: &mut AppState) {
    // Get terminal dimensions by drawing an empty frame
    let _ = terminal.draw(|frame| {
        let area = frame.area();
        // Initialize output_area with full terminal dimensions as a reasonable default.
        // This will be refined by the actual layout calculation during the first render.
        state.output.panel_areas.output_area.set(area);
    });
}
