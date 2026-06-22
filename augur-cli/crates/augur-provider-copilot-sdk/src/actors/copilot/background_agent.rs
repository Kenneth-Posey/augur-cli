//! Background SDK agent runner. Pattern follows `guided_plan::hooks::copilot_agent`.

use tokio::sync::mpsc;

use std::any::Any;
use std::sync::Arc;

use crate::actors::copilot::agent_feed_ops::{
    map_tool_complete_output, map_tool_progress_output, map_tool_start_output, ActiveToolCallMap,
    ToolInfo,
};
use crate::actors::copilot::background_event_mapper::{extract_llm_usage, map_background_event};
use augur_domain::background_events::{
    BackgroundEventClassifier, BackgroundPanelMode, DeltaAccumulator,
};
use augur_domain::newtypes::BufferThreshold;
use augur_domain::string_newtypes::{
    AccumulatedText, AgentName, ContentDelta, ModelLabel, OutputText, PromptText, ToolCallId,
};
use augur_domain::types::{AgentFeedOutput, FeedEntry, FeedId};
use augur_domain::StringNewtype;
use augur_domain::TokenTrackerHandle;

/// Static configuration for a background agent session.
///
/// Groups the agent identifier, initial prompt, and optional model override
/// so [`BackgroundAgentArgs`] stays within the 5-field limit.
/// Consumers: [`BackgroundAgentArgs`], [`run_background_agent`].
#[derive(bon::Builder)]
pub struct BackgroundAgentConfig {
    /// The agent type identifier to pass to the Copilot SDK session.
    pub agent: AgentName,
    /// Stable feed identifier for the UI transcript associated with this agent.
    pub feed_id: FeedId,
    /// The prompt to send to the background agent session.
    pub prompt: PromptText,
    /// Optional model display label override for this agent step.
    pub model: Option<ModelLabel>,
}

#[derive(bon::Builder)]
/// Arguments passed to [`run_background_agent`].
pub struct BackgroundAgentArgs {
    /// Static session config: agent identity, prompt, and optional model.
    pub config: BackgroundAgentConfig,
    /// Channel sender for emitting [`AgentFeedOutput`] events to the TUI feed panel.
    pub feed_tx: mpsc::Sender<FeedEntry>,
    /// Optional oneshot sender for transmitting the full accumulated response text.
    ///
    /// Only populated for real SDK runs; test doubles leave this as `None`.
    /// The final text is sent when the session completes normally (`SessionIdle`),
    /// allowing callers to extract a "pass"/"fail" signal from the agent's response.
    pub signal_tx: Option<tokio::sync::oneshot::Sender<AccumulatedText>>,
    /// Optional token-tracker handle for forwarding `AssistantUsage` data.
    ///
    /// When `Some`, each `SessionEventData::AssistantUsage` event is extracted into
    /// an `LlmUsage` and forwarded to the token-tracker actor via
    /// [`TokenTrackerHandle::record_usage`]. Fire-and-forget: dropped silently when
    /// the actor channel is full or the handle is `None`.
    pub token_tracker: Option<TokenTrackerHandle>,

    /// Provider-owned event classifier that maps SDK events to domain priority tiers.
    pub classifier: Arc<dyn BackgroundEventClassifier>,
}

async fn emit_feed_event(
    feed_tx: &mpsc::Sender<FeedEntry>,
    feed_id: &FeedId,
    event: AgentFeedOutput,
) {
    let _ = feed_tx
        .send(FeedEntry {
            feed_id: feed_id.clone(),
            output: event,
        })
        .await;
}

async fn emit_background_failure(args: &BackgroundAgentArgs, reason: OutputText) {
    tracing::warn!(agent = %args.config.agent, reason = %reason, "background agent failed");
    emit_feed_event(
        &args.feed_tx,
        &args.config.feed_id,
        AgentFeedOutput::TaskFailed {
            name: args.config.agent.clone(),
            reason,
        },
    )
    .await;
}

async fn start_background_client(args: &BackgroundAgentArgs) -> Option<copilot_sdk::Client> {
    let client = match build_background_client() {
        Ok(client) => client,
        Err(reason) => {
            emit_background_failure(args, OutputText::from(reason)).await;
            return None;
        }
    };
    if let Err(error) = client.start().await {
        emit_background_failure(
            args,
            OutputText::from(format!("failed to start Copilot client: {error}")),
        )
        .await;
        return None;
    }
    Some(client)
}

fn background_session_config(agent: &AgentName) -> copilot_sdk::SessionConfig {
    use crate::shared::copilot_permissions::allow_all_handler;
    use copilot_sdk::SessionConfig;

    let working_directory = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned));
    SessionConfig {
        agent: Some(agent.to_string()),
        streaming: true,
        config_dir: crate::shared::copilot_session_identity::isolated_config_dir(),
        working_directory,
        client_name: Some(
            crate::shared::copilot_session_identity::DCMK_COPILOT_CLIENT_NAME.to_string(),
        ),
        request_permission: Some(true),
        permission_handler: copilot_sdk::PermissionHandlerField::some(allow_all_handler()),
        ..Default::default()
    }
}

async fn create_background_session(
    client: &copilot_sdk::Client,
    args: &BackgroundAgentArgs,
) -> Option<std::sync::Arc<copilot_sdk::Session>> {
    match client
        .create_session(background_session_config(&args.config.agent))
        .await
    {
        Ok(session) => Some(session),
        Err(error) => {
            emit_background_failure(
                args,
                OutputText::from(format!("failed to create session: {error}")),
            )
            .await;
            let _ = client.stop().await;
            None
        }
    }
}

async fn send_background_prompt(
    session: &std::sync::Arc<copilot_sdk::Session>,
    args: &BackgroundAgentArgs,
) -> bool {
    match session.send(args.config.prompt.as_str()).await {
        Ok(_) => true,
        Err(error) => {
            emit_background_failure(
                args,
                OutputText::from(format!("failed to send prompt: {error}")),
            )
            .await;
            false
        }
    }
}

async fn stream_background_session(
    session: &std::sync::Arc<copilot_sdk::Session>,
    args: &mut BackgroundAgentArgs,
) {
    let mut sub = session.subscribe();
    if let Err(reason) = stream_to_feed(&mut sub, args).await {
        tracing::warn!(agent = %args.config.agent, reason = %reason, "stream_to_feed ended with error");
    }
}

async fn run_background_agent_with_sdk(mut args: BackgroundAgentArgs) {
    emit_feed_event(
        &args.feed_tx,
        &args.config.feed_id,
        AgentFeedOutput::TaskStarted {
            name: args.config.agent.clone(),
            model: args.config.model.clone(),
        },
    )
    .await;
    let Some(client) = start_background_client(&args).await else {
        return;
    };
    let Some(session) = create_background_session(&client, &args).await else {
        return;
    };
    if !send_background_prompt(&session, &args).await {
        let _ = session.destroy().await;
        client.stop().await;
        return;
    }
    stream_background_session(&session, &mut args).await;
    let _ = session.destroy().await;
    client.stop().await;
}

/// Build a `copilot_sdk::Client` configured for background agent sessions.
///
/// Locates the Copilot CLI via `find_copilot_cli()` and sets
/// `allow_all_tools: true` with `--allow-all` so the subprocess approves
/// built-in tools and path/URL permissions without blocking.
///
/// Returns an error string on failure for the caller to convert into
/// `AgentFeedOutput::TaskFailed`.
///
/// Consumers: `run_background_agent`.
fn build_background_client() -> Result<copilot_sdk::Client, String> {
    use copilot_sdk::ClientOptions;
    let cli_path = copilot_sdk::find_copilot_cli()
        .ok_or_else(|| "Copilot CLI not found in PATH".to_string())?;
    let cwd = std::env::current_dir().ok();
    let options = ClientOptions {
        cli_path: Some(cli_path),
        allow_all_tools: true,
        cli_args: Some(vec!["--allow-all".to_string()]),
        cwd,
        ..Default::default()
    };
    copilot_sdk::Client::new(options).map_err(|e| format!("failed to create Copilot client: {e}"))
}

/// Outcome of processing a single background stream event.
enum StreamStep {
    /// Keep the event loop running.
    Continue,
    /// Session is complete; exit the event loop.
    Done,
}

impl StreamStep {
    fn is_done(&self) -> bool {
        matches!(self, StreamStep::Done)
    }
}

/// Mutable accumulator state shared across per-event handler functions.
#[derive(bon::Builder)]
struct StreamContext {
    /// Delta accumulator for buffering and threshold-based flushing.
    accumulator: DeltaAccumulator,
    /// Full accumulated assistant text for signal extraction at session end.
    full_text: String,
    /// Registry of in-flight tool invocations for cross-event enrichment.
    tool_registry: ActiveToolCallMap,
}

/// Awaits the next event from the SDK subscription, emitting `TaskFailed` on channel close.
///
/// Returns `Err` if the subscription channel is closed unexpectedly.
async fn receive_next_event(
    sub: &mut copilot_sdk::EventSubscription,
    args: &mut BackgroundAgentArgs,
) -> Result<copilot_sdk::SessionEvent, OutputText> {
    match sub.recv().await {
        Ok(event) => Ok(event),
        Err(_) => {
            emit_feed_event(
                &args.feed_tx,
                &args.config.feed_id,
                AgentFeedOutput::TaskFailed {
                    name: args.config.agent.clone(),
                    reason: OutputText::from("session channel closed"),
                },
            )
            .await;
            Err(OutputText::from("session channel closed"))
        }
    }
}

/// Handles `AssistantMessageDelta`: accumulates full text and flushes buffered deltas.
///
/// Inputs: `d` - delta data; `ctx` - stream accumulator state; `args` - feed channel.
async fn handle_delta_event(
    d: &copilot_sdk::AssistantMessageDeltaData,
    ctx: &mut StreamContext,
    args: &BackgroundAgentArgs,
) {
    const DELTA_BUFFER_THRESHOLD: BufferThreshold = BufferThreshold(200);
    ctx.full_text.push_str(&d.delta_content);
    let flushed = ctx
        .accumulator
        .push(ContentDelta::new(&d.delta_content), DELTA_BUFFER_THRESHOLD);
    if let Some(flushed_text) = flushed {
        emit_feed_event(
            &args.feed_tx,
            &args.config.feed_id,
            AgentFeedOutput::StatusLine(OutputText::from(flushed_text.as_str())),
        )
        .await;
    }
}

/// Handles `AssistantMessage` boundary: flushes accumulator and emits `MessageBreak`.
///
/// Inputs: `ctx` - stream accumulator state; `args` - feed channel.
async fn handle_message_boundary(ctx: &mut StreamContext, args: &BackgroundAgentArgs) {
    if let Some(remaining) = ctx.accumulator.flush() {
        emit_feed_event(
            &args.feed_tx,
            &args.config.feed_id,
            AgentFeedOutput::StatusLine(OutputText::from(remaining.as_str())),
        )
        .await;
    }
    emit_feed_event(
        &args.feed_tx,
        &args.config.feed_id,
        AgentFeedOutput::MessageBreak,
    )
    .await;
}

/// Handles `ToolExecutionStart`: registers the tool invocation and emits the start event.
///
/// Inputs: `d` - tool start data; `ctx` - stream state; `args` - feed channel.
async fn handle_tool_start(
    d: &copilot_sdk::ToolExecutionStartData,
    ctx: &mut StreamContext,
    args: &BackgroundAgentArgs,
) {
    ctx.tool_registry.insert(
        ToolCallId::from(d.tool_call_id.as_str()),
        ToolInfo::from_start(d),
    );
    if let Some(output) = map_tool_start_output(d) {
        emit_feed_event(&args.feed_tx, &args.config.feed_id, output).await;
    }
}

/// Handles `ToolExecutionProgress`: emits the progress event if mappable.
///
/// Inputs: `d` - tool progress data; `args` - feed channel.
async fn handle_tool_progress(
    d: &copilot_sdk::ToolExecutionProgressData,
    args: &BackgroundAgentArgs,
) {
    if let Some(output) = map_tool_progress_output(d) {
        emit_feed_event(&args.feed_tx, &args.config.feed_id, output).await;
    }
}

/// Handles `ToolExecutionComplete`: emits enriched completion event using registry lookup.
///
/// Inputs: `d` - tool complete data; `ctx` - stream state for registry lookup; `args` - feed channel.
async fn handle_tool_complete(
    d: &copilot_sdk::ToolExecutionCompleteData,
    ctx: &StreamContext,
    args: &BackgroundAgentArgs,
) {
    if let Some(output) = map_tool_complete_output(d, &ctx.tool_registry) {
        emit_feed_event(&args.feed_tx, &args.config.feed_id, output).await;
    }
}

/// Handles `SessionIdle`: flushes state, delivers signal, emits `TaskCompleted`, returns `Done`.
///
/// Inputs: `ctx` - stream accumulator state; `args` - feed channel, signal sender, agent name.
async fn handle_session_idle(
    ctx: &mut StreamContext,
    args: &mut BackgroundAgentArgs,
) -> StreamStep {
    if let Some(final_text) = ctx.accumulator.flush() {
        emit_feed_event(
            &args.feed_tx,
            &args.config.feed_id,
            AgentFeedOutput::StatusLine(OutputText::from(final_text.as_str())),
        )
        .await;
    }
    if let Some(tx) = args.signal_tx.take() {
        let _ = tx.send(AccumulatedText::from(std::mem::take(&mut ctx.full_text)));
    }
    emit_feed_event(
        &args.feed_tx,
        &args.config.feed_id,
        AgentFeedOutput::TaskCompleted {
            name: args.config.agent.clone(),
        },
    )
    .await;
    StreamStep::Done
}

/// Handles `AssistantUsage`: forwards token usage to the tracker and emits the display event.
///
/// Inputs: `event` - raw SDK event; `ctx` - stream state for panel mode; `args` - handles.
async fn handle_usage_event(
    event: &copilot_sdk::SessionEvent,
    panel_mode: BackgroundPanelMode,
    args: &mut BackgroundAgentArgs,
) {
    if let Some(ref handle) = args.token_tracker
        && let Some(usage) = extract_llm_usage(&event.data)
    {
        handle.record_usage(usage);
    }
    if let Some(priority) = args.classifier.classify(&event.data as &dyn Any)
        && let Some(output) = map_background_event(&event.data, priority, panel_mode)
    {
        emit_feed_event(&args.feed_tx, &args.config.feed_id, output).await;
    }
}

async fn emit_priority_background_event(
    data: &copilot_sdk::SessionEventData,
    panel_mode: BackgroundPanelMode,
    args: &BackgroundAgentArgs,
) {
    if let Some(priority) = args.classifier.classify(data as &dyn Any)
        && let Some(output) = map_background_event(data, priority, panel_mode)
    {
        emit_feed_event(&args.feed_tx, &args.config.feed_id, output).await;
    }
}

async fn try_process_primary_stream_event(
    event: &copilot_sdk::SessionEvent,
    ctx: &mut StreamContext,
    args: &mut BackgroundAgentArgs,
) -> Option<StreamStep> {
    use copilot_sdk::SessionEventData;
    const PANEL_MODE: BackgroundPanelMode = BackgroundPanelMode::Normal;

    match &event.data {
        SessionEventData::AssistantMessageDelta(d) => handle_delta_event(d, ctx, args).await,
        SessionEventData::AssistantMessage(_) => handle_message_boundary(ctx, args).await,
        SessionEventData::ToolExecutionStart(d) => handle_tool_start(d, ctx, args).await,
        SessionEventData::ToolExecutionProgress(d) => handle_tool_progress(d, args).await,
        SessionEventData::ToolExecutionComplete(d) => handle_tool_complete(d, ctx, args).await,
        SessionEventData::SessionIdle(_) => return Some(handle_session_idle(ctx, args).await),
        SessionEventData::AssistantUsage(_) => handle_usage_event(event, PANEL_MODE, args).await,
        _ => return None,
    }
    Some(StreamStep::Continue)
}

/// Routes a single SDK event to the appropriate per-event handler.
///
/// Returns `StreamStep::Done` when `SessionIdle` is received (session complete).
/// Returns `StreamStep::Continue` for all other events.
async fn process_stream_event(
    event: &copilot_sdk::SessionEvent,
    ctx: &mut StreamContext,
    args: &mut BackgroundAgentArgs,
) -> StreamStep {
    const PANEL_MODE: BackgroundPanelMode = BackgroundPanelMode::Normal;
    if let Some(step) = try_process_primary_stream_event(event, ctx, args).await {
        return step;
    }
    emit_priority_background_event(&event.data, PANEL_MODE, args).await;
    StreamStep::Continue
}

/// Stream SDK session events to the agent feed channel with comprehensive event routing.
///
/// Each SDK event is classified by priority tier (Critical/Informational/Debug) and
/// routed through per-event handlers. `AssistantMessageDelta` content is buffered and
/// accumulated; on `SessionIdle` the full text is delivered via `args.signal_tx` and
/// `TaskCompleted` is emitted to close the feed panel entry.
///
/// Returns `Ok(())` on normal session completion (`SessionIdle`).
/// Returns `Err(OutputText)` if the subscription channel closes unexpectedly.
///
/// Consumers: `run_background_agent`.
pub(crate) async fn stream_to_feed(
    sub: &mut copilot_sdk::EventSubscription,
    args: &mut BackgroundAgentArgs,
) -> Result<(), OutputText> {
    let mut ctx = StreamContext::builder()
        .accumulator(DeltaAccumulator::default())
        .full_text(String::new())
        .tool_registry(ActiveToolCallMap::new())
        .build();
    loop {
        let event = receive_next_event(sub, args).await?;
        if process_stream_event(&event, &mut ctx, args).await.is_done() {
            return Ok(());
        }
    }
}

/// Runs a background Copilot SDK agent session, emitting status events on `feed_tx`.
///
/// 1. Sends `TaskStarted` on `feed_tx`.
/// 2. Builds a `copilot_sdk::Client`; on error, sends `TaskFailed` and returns.
/// 3. Starts the client; on error, sends `TaskFailed` and returns.
/// 4. Creates a session with `args.agent` and `streaming: true`.
/// 5. Sends `args.prompt` to the session.
/// 6. Streams events via `stream_to_feed` until the agent completes.
/// 7. On normal session completion, emits `TaskCompleted` to close the agent feed panel entry.
/// 8. Destroys the session and stops the client.
#[tracing::instrument(skip(args), level = "info")]
pub async fn run_background_agent(args: BackgroundAgentArgs) {
    run_background_agent_with_sdk(args).await;
}
