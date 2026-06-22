//! Feed routing logic: `FeedRouter` and `FeedChannels`.
//!
//! Routes SDK session events to the correct output channel: main conversation
//! feed or a background-agent feed. Symbols implemented in Phase 2 Step 3.

use crate::actors::copilot::agent_feed_ops::{
    ActiveToolCallMap, SubAgentState, TASK_TOOL_NAME, ToolInfo, advance_subagent_state,
    extract_active_task_id, map_custom_agent_completed, map_custom_agent_failed,
    map_custom_agent_started, map_sub_agent_delta_output, map_tool_complete_output,
    map_tool_progress_output, map_tool_start_output,
};
use crate::actors::copilot::event_mapper::map_event_to_output;
use augur_domain::ToolCallId;
use augur_domain::string_newtypes::{DisplayLine, EventType};
use augur_domain::types::{AgentFeedOutput, AgentOutput, FeedEntry, FeedId, RouteResult};
use copilot_sdk::{SessionEvent, SessionEventData};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

/// Returned by [`FeedChannels::send`] when the target channel's receiver has been dropped.
#[derive(Debug, PartialEq, Eq)]
pub struct FeedChannelClosed;

#[derive(bon::Builder)]
/// Routes `AgentFeedOutput` entries to the correct sender channel(s).
///
/// `single` constructs a router backed by one agent-feed channel. `send`
/// dispatches a `FeedEntry` to the channel that matches its `FeedId`,
/// returning `Ok(())` on success or no-op and `Err(FeedChannelClosed)` when
/// the channel is closed.
pub struct FeedChannels {
    agent_tx: mpsc::Sender<FeedEntry>,
    ask_tx: Option<mpsc::Sender<AgentFeedOutput>>,
}

impl FeedChannels {
    /// Create a `FeedChannels` backed by a single agent sender with no ask panel.
    ///
    /// The `ask_tx` slot is left empty; ask-panel events will be silently
    /// accepted (`true` returned) without being delivered.
    pub fn single(tx: mpsc::Sender<FeedEntry>) -> Self {
        FeedChannels {
            agent_tx: tx,
            ask_tx: None,
        }
    }

    /// Send a `FeedEntry` to the channel that matches its `FeedId`.
    ///
    /// - `FeedId::Agent(_)` → `agent_tx.send`.
    /// - `FeedId::AskPanel` → `ask_tx.send` when `Some`, else no-op `Ok(())`.
    /// - `FeedId::MainConversation` → no-op `Ok(())`.
    ///
    /// Returns `Err(FeedChannelClosed)` only when the target channel's receiver is dropped.
    pub async fn send(&self, entry: FeedEntry) -> Result<(), FeedChannelClosed> {
        match entry.feed_id {
            FeedId::Agent(_) => self
                .agent_tx
                .send(entry)
                .await
                .map_err(|_| FeedChannelClosed),
            FeedId::AskPanel => match &self.ask_tx {
                Some(tx) => tx.send(entry.output).await.map_err(|_| FeedChannelClosed),
                None => Ok(()),
            },
            FeedId::MainConversation => Ok(()),
        }
    }
}

#[derive(bon::Builder)]
/// Routes SDK session events to the main conversation feed or a background-agent feed.
///
/// Maintains `SubAgentState`, an `ActiveToolCallMap`, and an `active_agents` map to
/// determine per-event routing. `route_event` is the single public entry point:
/// it advances state, applies suppression rules for `main_out`, and selects the
/// target feed for `feed_out`.
pub struct FeedRouter {
    sub_agent_state: SubAgentState,
    tool_registry: ActiveToolCallMap,
    active_agents: HashMap<ToolCallId, FeedId>,
    started_agents: HashSet<ToolCallId>,
}

impl Default for FeedRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl FeedRouter {
    /// Create a new `FeedRouter` in the initial `Idle` state with empty registries.
    pub fn new() -> Self {
        FeedRouter::builder()
            .sub_agent_state(SubAgentState::Idle)
            .tool_registry(ActiveToolCallMap::new())
            .active_agents(HashMap::new())
            .started_agents(HashSet::new())
            .build()
    }

    /// Route a single SDK session event and return main and agent-feed outputs.
    ///
    /// Steps: update registries, snapshot pre-advance state, advance state machine,
    /// compute feed id, compute `main_out` with suppression rules, compute `feed_out`.
    pub fn route_event(&mut self, event: &SessionEvent) -> RouteResult {
        let event_kind = debug_event_kind(&event.data);
        let pre_state = self.sub_agent_state.clone();
        self.update_registries(&event.data);
        let pre_advance = self.sub_agent_state.clone();
        advance_subagent_state(&event.data, &mut self.sub_agent_state);
        let feed_id = self.compute_feed_id(&event.data);
        let main_out = self.compute_main_out(&event.data, &pre_advance);
        let feed_out = self.compute_feed_out(event, feed_id);
        let route = RouteResult { main_out, feed_out };
        tracing::info!(
            %event_kind,
            pre_state = ?pre_state,
            pre_advance_state = ?pre_advance,
            post_state = ?self.sub_agent_state,
            main_out = route.main_out.is_some(),
            feed_out = route.feed_out.is_some(),
            feed_id = %route.feed_out.as_ref().map(|entry| debug_feed_id(&entry.feed_id)).unwrap_or_else(|| DisplayLine::from("none")),
            "copilot.feed_router.route_event"
        );
        route
    }

    /// Update the tool registry and active-agent map from an incoming event.
    ///
    /// `ToolExecutionStart`: registers `ToolInfo` for all tools; also inserts a
    /// `FeedId::Agent` entry into `active_agents` for `"task"` tool calls.
    /// `ToolExecutionComplete` while state is `AwaitingCompletion` for that id:
    /// removes the completed agent from `active_agents`.
    fn update_registries(&mut self, data: &SessionEventData) {
        use SessionEventData as E;
        match data {
            E::ToolExecutionStart(d) => {
                let tool_call_id = ToolCallId::from(d.tool_call_id.as_str());
                self.tool_registry
                    .insert(tool_call_id.clone(), ToolInfo::from_start(d));
                if d.tool_name == TASK_TOOL_NAME {
                    self.active_agents.insert(
                        ToolCallId::from(d.tool_call_id.as_str()),
                        FeedId::Agent(tool_call_id),
                    );
                }
            }
            E::ToolExecutionComplete(d) => {
                self.started_agents
                    .remove(&ToolCallId::from(d.tool_call_id.as_str()));
                let is_awaiting = matches!(
                    &self.sub_agent_state,
                    SubAgentState::AwaitingCompletion(id) if id == &d.tool_call_id
                );
                if is_awaiting {
                    self.active_agents
                        .remove(&ToolCallId::from(d.tool_call_id.as_str()));
                }
            }
            E::UserMessage(_) => {
                // New top-level user turn: recover from any stale background-agent
                // routing state so the next no-parent assistant output returns to main.
                tracing::info!(
                    prev_state = ?self.sub_agent_state,
                    active_agents = self.active_agents.len(),
                    "copilot.feed_router.user_message_reset"
                );
                self.sub_agent_state = SubAgentState::Idle;
                self.active_agents.clear();
                self.started_agents.clear();
            }
            _ => {}
        }
    }

    /// Determine the target feed id for an incoming event, if any.
    ///
    /// Priority order:
    /// 1. `parent_tool_call_id` lookup in `active_agents` (fallback: `Agent(pid)`).
    /// 2. Custom-agent lifecycle variants (`CustomAgentStarted/Completed/Failed`).
    /// 3. `AgentActive` state: active task id.
    /// 4. Default: `None` (main-session event).
    fn compute_feed_id(&self, data: &SessionEventData) -> Option<FeedId> {
        use SessionEventData as E;
        if let Some(pid) = extract_parent_id(data) {
            return self
                .active_agents
                .get(&ToolCallId::from(pid))
                .cloned()
                .or_else(|| Some(FeedId::Agent(ToolCallId::from(pid))));
        }
        match data {
            E::CustomAgentStarted(d) => {
                return Some(FeedId::Agent(ToolCallId::from(d.tool_call_id.as_str())));
            }
            E::CustomAgentCompleted(d) => {
                return Some(FeedId::Agent(ToolCallId::from(d.tool_call_id.as_str())));
            }
            E::CustomAgentFailed(d) => {
                return Some(FeedId::Agent(ToolCallId::from(d.tool_call_id.as_str())));
            }
            _ => {}
        }
        if matches!(self.sub_agent_state, SubAgentState::AgentActive(_)) {
            let id = extract_active_task_id(&self.sub_agent_state);
            return Some(FeedId::Agent(ToolCallId::from(id.as_str())));
        }
        None
    }

    /// Compute the main-feed output, applying per-variant suppression rules.
    ///
    /// Uses the pre-advance state for `ToolExecutionComplete` suppression (so the
    /// outer task completion is hidden while `AwaitingCompletion`); uses the
    /// post-advance state (`self.sub_agent_state`) for all other events. Parent-
    /// scoped events stay out of the main feed, but background lifecycle state
    /// alone does not suppress assistant deltas, assistant boundaries, or idle
    /// completion.
    fn compute_main_out(
        &self,
        data: &SessionEventData,
        pre_advance: &SubAgentState,
    ) -> Option<AgentOutput> {
        use SessionEventData as E;
        let effective = match data {
            E::ToolExecutionComplete(_) => pre_advance,
            _ => &self.sub_agent_state,
        };
        let has_parent = extract_parent_id(data).is_some();
        if suppressed_from_main(data, effective, has_parent) {
            return None;
        }
        map_event_to_output(data)
    }

    /// Compute the agent-feed output for an event, if a target feed was identified.
    ///
    /// Returns `None` immediately when `feed_id` is `None`. Otherwise maps the
    /// event variant to an `AgentFeedOutput` and wraps it in a `FeedEntry`.
    /// `_pre_advance` is accepted for API consistency but not used in the body.
    fn compute_feed_out(
        &mut self,
        event: &SessionEvent,
        feed_id: Option<FeedId>,
    ) -> Option<FeedEntry> {
        let id = feed_id?;
        // Suppress duplicate TaskStarted for multi-turn agents by tool_call_id.
        // This remains correct under interleaved parallel starts because each
        // task keeps its own id independent of the single lifecycle state value.
        if let SessionEventData::CustomAgentStarted(d) = &event.data {
            let tool_call_id = ToolCallId::from(d.tool_call_id.as_str());
            if self.started_agents.contains(&tool_call_id) {
                return None;
            }
            self.started_agents.insert(tool_call_id);
        }
        let output = map_event_to_feed_output(&event.data, &self.tool_registry)?;
        Some(FeedEntry {
            feed_id: id,
            output,
        })
    }
}

/// Return `true` when `data` should be suppressed from the main conversation feed.
///
/// `state` is the already-resolved effective state: `pre_advance` for
/// `ToolExecutionComplete`, `sub_agent_state` (post-advance) for all others.
/// `has_parent` suppresses any event that carries a `parent_tool_call_id`.
/// Returns `true` when `ToolExecutionStart` or `ToolExecutionComplete` events should
/// be suppressed from the main feed.
///
/// Only tool execution routing is state-dependent here; assistant deltas,
/// assistant boundaries, and idle completion should still reach the main feed
/// even while a background lifecycle is active.
fn is_tool_execution_suppressed(state: &SubAgentState, has_parent: bool) -> bool {
    has_parent
        || matches!(
            state,
            SubAgentState::TaskPending(_)
                | SubAgentState::AgentActive(_)
                | SubAgentState::AwaitingCompletion(_)
        )
}

fn suppressed_from_main(data: &SessionEventData, state: &SubAgentState, has_parent: bool) -> bool {
    use SessionEventData as E;
    match data {
        E::AssistantMessageDelta(_) | E::AssistantMessage(_) | E::SessionIdle(_) => has_parent,
        E::ToolExecutionStart(_) | E::ToolExecutionComplete(_) => {
            is_tool_execution_suppressed(state, has_parent)
        }
        E::ToolExecutionProgress(_) | E::ToolExecutionPartialResult(_) => {
            has_parent || matches!(state, SubAgentState::AgentActive(_))
        }
        _ => false,
    }
}

/// Map a `SessionEventData` variant to an `AgentFeedOutput` for the agent feed.
///
/// Returns `None` for variants that have no agent-feed representation.
fn map_event_to_feed_output(
    data: &SessionEventData,
    registry: &ActiveToolCallMap,
) -> Option<AgentFeedOutput> {
    map_custom_agent_feed_output(data).or_else(|| map_tool_or_message_feed_output(data, registry))
}

fn map_custom_agent_feed_output(data: &SessionEventData) -> Option<AgentFeedOutput> {
    use SessionEventData as E;
    match data {
        E::CustomAgentStarted(d) => Some(map_custom_agent_started(d)),
        E::CustomAgentCompleted(d) => Some(map_custom_agent_completed(d)),
        E::CustomAgentFailed(d) => Some(map_custom_agent_failed(d)),
        _ => None,
    }
}

fn map_tool_or_message_feed_output(
    data: &SessionEventData,
    registry: &ActiveToolCallMap,
) -> Option<AgentFeedOutput> {
    use SessionEventData as E;
    match data {
        E::AssistantMessageDelta(d) => map_sub_agent_delta_output(d),
        E::AssistantMessage(_) => Some(AgentFeedOutput::MessageBreak),
        E::ToolExecutionStart(d) => map_tool_start_output(d),
        E::ToolExecutionComplete(d) => map_tool_complete_output(d, registry),
        E::ToolExecutionProgress(d) => map_tool_progress_output(d),
        E::ToolExecutionPartialResult(_) => None,
        _ => None,
    }
}

/// Extract the `parent_tool_call_id` from an event variant that carries one.
///
/// Returns `d.parent_tool_call_id.as_deref()` for `AssistantMessageDelta`,
/// `ToolExecutionStart`, and `ToolExecutionComplete` - the three SDK variants
/// that have a `parent_tool_call_id: Option<String>` field. Returns `None` for
/// all other variants.
fn extract_parent_id(data: &SessionEventData) -> Option<&str> {
    use SessionEventData as E;
    match data {
        E::AssistantMessageDelta(d) => d.parent_tool_call_id.as_deref(),
        E::ToolExecutionStart(d) => d.parent_tool_call_id.as_deref(),
        E::ToolExecutionComplete(d) => d.parent_tool_call_id.as_deref(),
        _ => None,
    }
}

/// Return a static string label for tool-execution event kinds.
///
/// Returns `Some(label)` for the four `ToolExecution*` variants; `None` for all others.
/// Consumers: [`debug_event_kind`].
fn debug_tool_event_kind(data: &SessionEventData) -> Option<&'static str> {
    use SessionEventData as E;
    match data {
        E::ToolExecutionStart(_) => Some("ToolExecutionStart"),
        E::ToolExecutionComplete(_) => Some("ToolExecutionComplete"),
        E::ToolExecutionProgress(_) => Some("ToolExecutionProgress"),
        E::ToolExecutionPartialResult(_) => Some("ToolExecutionPartialResult"),
        _ => None,
    }
}

/// Return a static string label for custom-agent event kinds.
///
/// Returns `Some(label)` for the three `CustomAgent*` variants; `None` for all others.
/// Consumers: [`debug_event_kind`].
fn debug_agent_event_kind(data: &SessionEventData) -> Option<&'static str> {
    use SessionEventData as E;
    match data {
        E::CustomAgentStarted(_) => Some("CustomAgentStarted"),
        E::CustomAgentCompleted(_) => Some("CustomAgentCompleted"),
        E::CustomAgentFailed(_) => Some("CustomAgentFailed"),
        _ => None,
    }
}

/// Return a static string label for the event kind, used in tracing context fields.
///
/// Delegates tool-execution variants to [`debug_tool_event_kind`] and custom-agent
/// variants to [`debug_agent_event_kind`]; handles core message variants inline.
/// Consumers: [`FeedRouter::route_event`], `session_ops` tracing spans.
pub(crate) fn debug_event_kind(data: &SessionEventData) -> EventType {
    use SessionEventData as E;
    if let Some(kind) = debug_tool_event_kind(data) {
        return EventType::from(kind);
    }
    if let Some(kind) = debug_agent_event_kind(data) {
        return EventType::from(kind);
    }
    match data {
        E::UserMessage(_) => EventType::from("UserMessage"),
        E::AssistantMessageDelta(_) => EventType::from("AssistantMessageDelta"),
        E::AssistantMessage(_) => EventType::from("AssistantMessage"),
        E::SessionIdle(_) => EventType::from("SessionIdle"),
        _ => EventType::from("Other"),
    }
}

/// Format a `FeedId` as a display string for tracing context fields.
///
/// Consumers: [`FeedRouter::route_event`], `session_ops` tracing spans.
pub(crate) fn debug_feed_id(feed_id: &FeedId) -> DisplayLine {
    match feed_id {
        FeedId::MainConversation => DisplayLine::from("MainConversation"),
        FeedId::AskPanel => DisplayLine::from("AskPanel"),
        FeedId::Agent(id) => DisplayLine::from(format!("Agent({id})")),
    }
}
