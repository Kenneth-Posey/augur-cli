//! Pure mapping from `copilot_sdk::SessionEvent` to `AgentFeedOutput`.
//!
//! Contains no I/O, no channels, and no actor state. Each function is a
//! pure transformation over SDK data types. Phase 2 calls these functions
//! from the async dispatch loop. Gated on `copilot-executor` because it
//! uses SDK types.

use augur_domain::string_newtypes::{AgentName, OutputText, ToolCallId, ToolName};
use augur_domain::tool_call_formatting::format_tool_call_line;
use augur_domain::types::AgentFeedOutput;
use augur_domain::StringNewtype;
use copilot_sdk::{
    AssistantMessageDeltaData, CustomAgentCompletedData, CustomAgentFailedData,
    CustomAgentStartedData, SessionEventData, ToolExecutionCompleteData, ToolExecutionProgressData,
    ToolExecutionStartData,
};
use std::collections::HashMap;

/// Tool name used by the Copilot SDK for spawning background agents.
pub const TASK_TOOL_NAME: &str = "task";

/// Metadata about a tool call captured at start time, keyed by `tool_call_id`.
///
/// Created from `ToolExecutionStartData` and stored in `ActiveToolCallMap` so
/// `map_tool_complete_output` can display the tool name and description instead of
/// the raw `tool_call_id`.
pub struct ToolInfo {
    /// SDK name of the tool (e.g. `"bash"`, `"read_file"`).
    pub tool_name: ToolName,
    /// Human-readable description extracted from `arguments["description"]`,
    /// if the caller provided one.
    pub description: Option<String>,
}

impl ToolInfo {
    /// Extract `tool_name` and `description` from a `ToolExecutionStartData`.
    ///
    /// `description` is read from `arguments["description"]` as a JSON string.
    /// Returns `None` for description if `arguments` is absent or has no
    /// string-typed `"description"` key.
    pub fn from_start(d: &ToolExecutionStartData) -> Self {
        let description = d
            .arguments
            .as_ref()
            .and_then(|args| args.get("description"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        ToolInfo {
            tool_name: ToolName::from(d.tool_name.as_str()),
            description,
        }
    }
}

/// Correlates `tool_call_id` values to `ToolInfo` for the current dispatch loop.
///
/// Populated on `ToolExecutionStart`; queried on `ToolExecutionComplete` so
/// the complete handler can format output with the tool name and description
/// rather than the raw `tool_call_id`.
#[derive(Default)]
pub struct ActiveToolCallMap(HashMap<ToolCallId, ToolInfo>);

impl ActiveToolCallMap {
    /// Create an empty map.
    pub fn new() -> Self {
        ActiveToolCallMap(HashMap::new())
    }

    /// Insert a `ToolInfo` entry keyed by `tool_call_id`.
    pub fn insert(&mut self, id: ToolCallId, info: ToolInfo) {
        self.0.insert(id, info);
    }

    /// Look up tool info by `tool_call_id`, returning `None` if absent.
    pub fn get(&self, id: &ToolCallId) -> Option<&ToolInfo> {
        self.0.get(id)
    }
}

/// Runtime state of the sub-agent lifecycle, used as the routing predicate
/// for the event dispatch loop.
///
/// The state machine progresses through four stages:
/// - `Idle`: no background agent is executing; all events route normally.
/// - `TaskPending(tool_call_id)`: a `task` tool execution has started but
///   `CustomAgentStarted` has not yet fired; suppress the task tool start event.
/// - `AgentActive(tool_call_id)`: between `CustomAgentStarted` and
///   `CustomAgentCompleted/Failed`; route deltas to the feed, suppress task
///   tool progress/partial results from main chat.
/// - `AwaitingCompletion(tool_call_id)`: `CustomAgentCompleted/Failed` fired
///   but the matching `ToolExecutionComplete` has not; suppress it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubAgentState {
    Idle,
    TaskPending(String),
    AgentActive(String),
    AwaitingCompletion(String),
}

/// Map a `CustomAgentStarted` event to `TaskStarted`.
///
/// Uses `agent_display_name` as the human-readable name shown in the
/// `AgentFeed` TUI panel. Pure; no I/O.
pub fn map_custom_agent_started(d: &CustomAgentStartedData) -> AgentFeedOutput {
    AgentFeedOutput::TaskStarted {
        name: AgentName::from(d.agent_display_name.as_str()),
        model: None,
    }
}

/// Map a `CustomAgentCompleted` event to `TaskCompleted`.
///
/// Uses `agent_name` as the identifier. Pure; no I/O.
pub fn map_custom_agent_completed(d: &CustomAgentCompletedData) -> AgentFeedOutput {
    AgentFeedOutput::TaskCompleted {
        name: AgentName::from(d.agent_name.as_str()),
    }
}

/// Map a `CustomAgentFailed` event to `TaskFailed`.
///
/// Uses `agent_name` as the identifier and `error` as the failure reason.
/// Pure; no I/O.
pub fn map_custom_agent_failed(d: &CustomAgentFailedData) -> AgentFeedOutput {
    AgentFeedOutput::TaskFailed {
        name: AgentName::from(d.agent_name.as_str()),
        reason: OutputText::from(d.error.as_str()),
    }
}

/// Map an `AssistantMessageDelta` to a `StatusLine` for the agent feed.
///
/// Returns `Some(StatusLine(...))` when `delta_content` is non-empty.
/// Stateless - callers must apply any state gate before calling.
///
/// Parameters:
/// - `d`: the delta data payload from the SDK event.
pub fn map_sub_agent_delta_output(d: &AssistantMessageDeltaData) -> Option<AgentFeedOutput> {
    if d.delta_content.is_empty() {
        return None;
    }
    Some(AgentFeedOutput::StatusLine(OutputText::from(
        d.delta_content.as_str(),
    )))
}

/// Map a `ToolExecutionStart` event to a `ToolEventLine` for the agent feed.
///
/// Uses the shared main-feed formatter so tool-call labels stay identical in both
/// panes, including multiline detail rows and `file_create` preview truncation.
///
/// Stateless - callers must apply any state gate before calling.
///
/// Parameters:
/// - `d`: the tool execution start data payload.
pub fn map_tool_start_output(d: &ToolExecutionStartData) -> Option<AgentFeedOutput> {
    let args = d.arguments.clone().unwrap_or(serde_json::Value::Null);
    let label = format_tool_call_line(ToolName::from(d.tool_name.as_str()), &args);
    Some(AgentFeedOutput::ToolEventLine(label))
}

/// Map a `ToolExecutionProgress` event to a `ToolEventLine` for the agent feed.
///
/// Always emits `Some(ToolEventLine(progress_message))`. Stateless - callers
/// must apply any state gate before calling.
///
/// Parameters:
/// - `d`: the tool execution progress data payload.
pub fn map_tool_progress_output(d: &ToolExecutionProgressData) -> Option<AgentFeedOutput> {
    Some(AgentFeedOutput::ToolEventLine(OutputText::from(
        d.progress_message.as_str(),
    )))
}

/// Map a `ToolExecutionComplete` event to a `ToolEventLine` for the agent feed.
///
/// Looks up the `tool_call_id` in `registry` for the tool name and description.
/// Emits `"✓ {name}: {desc}"` on success or `"✗ {name}: {error}"` on failure.
/// Stateless - callers must apply any state gate before calling.
///
/// Parameters:
/// - `d`: the tool execution complete data payload.
/// - `registry`: registry populated at `ToolExecutionStart` time.
pub fn map_tool_complete_output(
    d: &ToolExecutionCompleteData,
    registry: &ActiveToolCallMap,
) -> Option<AgentFeedOutput> {
    let tool_id = ToolCallId::from(d.tool_call_id.as_str());
    let info = registry.get(&tool_id);
    let label = format_tool_complete_label(d, info);
    Some(AgentFeedOutput::ToolEventLine(OutputText::from(
        label.as_str(),
    )))
}

/// Format the status line text for a `ToolExecutionComplete` event.
///
/// Resolves the tool name from `info` (falling back to `tool_call_id`),
/// then builds the `✓` or `✗` prefixed string with optional description or
/// error message. Called by `map_tool_complete_output`.
fn format_tool_complete_label(d: &ToolExecutionCompleteData, info: Option<&ToolInfo>) -> String {
    let symbol = if d.success { '✓' } else { '✗' };
    let name = info
        .map(|i| i.tool_name.as_str())
        .unwrap_or(d.tool_call_id.as_str());
    if d.success {
        match info.and_then(|i| i.description.as_deref()) {
            Some(desc) => format!("{symbol} {name}: {desc}"),
            None => format!("{symbol} {name}"),
        }
    } else {
        match d.error.as_ref() {
            Some(err) => format!("{symbol} {name}: {}", err.message),
            None => format!("{symbol} {name}"),
        }
    }
}

/// Extract the active `tool_call_id` string from a `SubAgentState`.
///
/// Returns the id from whichever carrying variant is active, or an empty
/// `String` for `Idle`. Used by `advance_subagent_state` to forward the
/// id across state transitions without duplicating nested match arms.
/// Also used by `feed_router::FeedRouter::compute_feed_id` to read the
/// active task id when state is `AgentActive`.
pub(crate) fn extract_active_task_id(state: &SubAgentState) -> String {
    match state {
        SubAgentState::TaskPending(id)
        | SubAgentState::AgentActive(id)
        | SubAgentState::AwaitingCompletion(id) => id.clone(),
        SubAgentState::Idle => String::new(),
    }
}

/// Advance the sub-agent lifecycle state based on the incoming SDK event.
///
/// Drives the four-stage state machine:
/// - `ToolExecutionStart("task")` → `TaskPending(tool_call_id)`
/// - `CustomAgentStarted` → `AgentActive(tool_call_id)` (id preserved from `TaskPending`)
/// - `CustomAgentCompleted` / `CustomAgentFailed` → `AwaitingCompletion(tool_call_id)`
/// - `ToolExecutionComplete` (matching id or empty fallback) → `Idle`
///
/// All other events leave the state unchanged.
pub(crate) fn advance_subagent_state(data: &SessionEventData, state: &mut SubAgentState) {
    use SessionEventData as E;
    match data {
        E::ToolExecutionStart(d) if d.tool_name == TASK_TOOL_NAME => {
            *state = SubAgentState::TaskPending(d.tool_call_id.clone());
        }
        E::CustomAgentStarted(_) => {
            *state = SubAgentState::AgentActive(extract_active_task_id(state));
        }
        E::CustomAgentCompleted(_) | E::CustomAgentFailed(_) => {
            *state = SubAgentState::AwaitingCompletion(extract_active_task_id(state));
        }
        E::ToolExecutionComplete(d) => {
            if matches!(
                state,
                SubAgentState::TaskPending(id)
                    | SubAgentState::AgentActive(id)
                    | SubAgentState::AwaitingCompletion(id)
                    if id.is_empty() || *id == d.tool_call_id
            ) {
                *state = SubAgentState::Idle;
            }
        }
        _ => {}
    }
}
