//! Background event mapper for Copilot actor.
//!
//! This module implements tier-based filtering and character limits for SessionEventData
//! to AgentFeedOutput mapping. Events are categorized as Critical, Informational, or Debug
//! based on their importance and impact on the user experience.
//!
//! Character limits enforce conciseness:
//! - Critical events: 200 characters (user-facing errors, session state changes)
//! - Informational events: 100 characters (user actions, assistant progress)
//! - Debug events: 50 characters (internal diagnostics, verbose traces)
//!
//! The `BackgroundPanelMode` determines which event tiers are displayed:
//! - `Critical`: only Critical tier events
//! - `Normal`: Critical + Informational tiers
//! - `Debug`: all tiers (Critical, Informational, Debug)

use augur_domain::background_events::{BackgroundEventPriority, BackgroundPanelMode};
use augur_domain::newtypes::{IsPredicate, NumericNewtype, TokenCount};
use augur_domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::types::{AgentFeedOutput, LlmTokenCounts, LlmUsage};
use copilot_sdk::SessionEventData;

/// Maximum character length for Critical-tier background events.
const CRITICAL_CHAR_LIMIT: usize = 200;

/// Maximum character length for Informational-tier background events.
const INFORMATIONAL_CHAR_LIMIT: usize = 100;

/// Maximum character length for Debug-tier background events.
const DEBUG_CHAR_LIMIT: usize = 50;

/// Type alias for background event mapping results.
pub type EventMapResult = Option<AgentFeedOutput>;

/// Combined mapping result carrying both a display event and optional structured usage.
///
/// Returned by [`map_background_event_with_usage`] so callers can forward
/// `LlmUsage` to the token-tracker actor without a second pass over the event.
///
/// # Fields
///
/// - `display` - the mapped `AgentFeedOutput` (identical to what
///   [`map_background_event`] would have returned). `None` when the event
///   produces no visible output or is filtered out by the active mode.
/// - `usage` - structured token and cost data. `Some` iff the source event
///   is `SessionEventData::AssistantUsage`. `None` for all other variants.
pub(crate) struct BackgroundMappedEvent {
    /// Mapped display output, or `None` when the event should not be shown.
    pub display: Option<AgentFeedOutput>,
    /// Extracted usage data. `Some` only for `AssistantUsage` events.
    pub usage: Option<LlmUsage>,
}

/// Maps a SessionEventData to an AgentFeedOutput for display in the background panel.
///
/// This function implements the core logic for transforming Copilot SDK session events
/// into user-facing feedback text for display in the background panel. The transformation
/// includes three stages:
///
/// 1. **Routing**: Each [`SessionEventData`] variant is routed to appropriate text output based
///    on its semantic category (Critical, Informational, or Debug events).
///
/// 2. **Filtering**: Based on the current [`BackgroundPanelMode`], events may be filtered out
///    entirely if their priority tier is not enabled:
///    - `Critical` mode: shows only Critical events
///    - `Normal` mode: shows Critical and Informational events
///    - `Debug` mode: shows all events (Critical, Informational, Debug)
///
/// 3. **Truncation**: Event text is truncated to tier-specific character limits to ensure
///    concise display:
///    - Critical: 200 characters (user-facing errors, session state changes)
///    - Informational: 100 characters (user actions, assistant progress)
///    - Debug: 50 characters (internal diagnostics, verbose traces)
///
/// # Arguments
///
/// * `event` - The [`SessionEventData`] event to map
/// * `priority` - The [`BackgroundEventPriority`] tier of this event
/// * `mode` - The current [`BackgroundPanelMode`] determining visibility
///
/// # Returns
///
/// `Some(AgentFeedOutput::StatusLine)` containing the mapped and truncated event text if the
/// event should be displayed, or `None` if:
/// - The event priority is filtered out by the current mode
/// - The event is unmappable (e.g., `SessionUsageInfo`, `Unknown` variants)
///
/// # Examples
///
/// Map a user message event in Normal mode:
/// ```ignore
/// let event = SessionEventData::UserMessage(user_message_data);
/// let output = map_background_event(
///     &event,
///     BackgroundEventPriority::Informational,
///     BackgroundPanelMode::Normal,
/// );
/// assert!(output.is_some());
/// ```
///
/// Map a debug event when only Critical mode is active:
/// ```ignore
/// let event = SessionEventData::SessionResume(resume_data);
/// let output = map_background_event(
///     &event,
///     BackgroundEventPriority::Debug,
///     BackgroundPanelMode::Critical,  // Debug events filtered out
/// );
/// assert!(output.is_none());
/// ```
pub fn map_background_event(
    event: &SessionEventData,
    priority: BackgroundEventPriority,
    mode: BackgroundPanelMode,
) -> Option<AgentFeedOutput> {
    if !should_emit(mode, priority).0 {
        return None;
    }

    let text = match priority {
        BackgroundEventPriority::Critical => map_critical_text(event),
        BackgroundEventPriority::Informational => map_informational_text(event),
        BackgroundEventPriority::Debug => map_debug_text(event),
    }?;

    let limit = match priority {
        BackgroundEventPriority::Critical => CRITICAL_CHAR_LIMIT,
        BackgroundEventPriority::Informational => INFORMATIONAL_CHAR_LIMIT,
        BackgroundEventPriority::Debug => DEBUG_CHAR_LIMIT,
    };

    let truncated = truncate_to_limit(&text, limit);
    Some(AgentFeedOutput::StatusLine(OutputText::from(
        truncated.as_str(),
    )))
}

/// Maps Critical-tier events to display text.
///
/// Returns `None` for unmappable variants (e.g. `PermissionRequested` with
/// an unknown permission string) or for non-critical event variants.
fn map_critical_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::SessionStart(_) => Some("Session started".to_string()),
        SessionEventData::SessionError(d) => Some(format!("Error: {}", d.message)),
        SessionEventData::SessionShutdown(_) => Some("Session shutdown".to_string()),
        SessionEventData::Abort(d) => Some(format!("Aborted: {}", d.reason)),
        SessionEventData::CustomAgentFailed(d) => {
            Some(format!("Agent {} failed: {}", d.agent_name, d.error))
        }
        SessionEventData::PermissionRequested(d) => {
            let perm = d
                .permission_request
                .as_ref()
                .and_then(|p| p.get("permission"))
                .and_then(|p| p.as_str())
                .unwrap_or("unknown");
            if perm == "unknown" {
                return None;
            }
            Some(format!("Permission: {}", perm))
        }
        _ => None,
    }
}

/// Maps Informational-tier events to display text by delegating to focused sub-helpers.
///
/// Returns `None` for variants not in the Informational tier.
fn map_informational_text(event: &SessionEventData) -> Option<String> {
    map_agent_message_text(event).or_else(|| map_tool_interaction_text(event))
}

/// Maps assistant/agent message events to display text.
///
/// Handles: UserMessage, AssistantTurnStart, AssistantIntent, AssistantMessage,
/// AssistantMessageDelta, AssistantTurnEnd, CustomAgent*, SessionHandoff.
fn map_agent_message_text(event: &SessionEventData) -> Option<String> {
    map_core_agent_message_text(event).or_else(|| map_custom_agent_message_text(event))
}

fn map_core_agent_message_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::UserMessage(d) => Some(format!("\u{2192} {}", d.content)),
        SessionEventData::AssistantTurnStart(_) => Some("[Assistant thinking...]".to_string()),
        SessionEventData::AssistantIntent(d) => Some(format_intent(&d.intent)),
        SessionEventData::AssistantMessage(d) => Some(d.content.clone()),
        SessionEventData::AssistantMessageDelta(d) => Some(d.delta_content.clone()),
        SessionEventData::AssistantTurnEnd(_) => None,
        _ => None,
    }
}

fn map_custom_agent_message_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::CustomAgentStarted(d) => Some(format!("Agent {} started", d.agent_name)),
        SessionEventData::CustomAgentCompleted(d) => {
            Some(format!("Agent {} completed", d.agent_name))
        }
        SessionEventData::CustomAgentSelected(d) => Some(format!("Using: {}", d.agent_name)),
        SessionEventData::SessionHandoff(_) => Some("\u{2192} Agent handoff".to_string()),
        _ => None,
    }
}

/// Maps tool interaction and hook events to display text.
///
/// Handles: ToolUserRequested, ToolExecution*, HookStart, HookEnd, SkillInvoked,
/// ExternalToolRequested. Returns `None` for hook events targeting `postToolUse`.
fn map_tool_interaction_text(event: &SessionEventData) -> Option<String> {
    map_tool_execution_event_text(event)
        .or_else(|| map_hook_or_skill_event_text(event))
        .or_else(|| map_external_tool_event_text(event))
}

fn map_tool_execution_event_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::ToolUserRequested(d) => Some(format!("Tool requested: {}", d.tool_name)),
        SessionEventData::ToolExecutionStart(d) => {
            Some(format_tool_with_args(&d.tool_name, d.arguments.as_ref()))
        }
        SessionEventData::ToolExecutionComplete(d) => Some(format_tool_event("", None, Some(d))),
        SessionEventData::ToolExecutionProgress(d) => {
            Some(format!("\u{2192} {}", d.progress_message))
        }
        _ => None,
    }
}

fn map_hook_or_skill_event_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::HookStart(d) => map_hook_event_text(&d.hook_type, "hook"),
        SessionEventData::HookEnd(d) => map_hook_event_text(&d.hook_type, "complete"),
        SessionEventData::SkillInvoked(d) => Some(format!("Skill: {}", d.name)),
        _ => None,
    }
}

fn map_external_tool_event_text(event: &SessionEventData) -> Option<String> {
    if let SessionEventData::ExternalToolRequested(d) = event {
        let tool = d.tool_name.as_deref().unwrap_or("unknown");
        Some(format!("External tool: {}", tool))
    } else {
        None
    }
}

/// Formats a hook event as a status line, or returns `None` for `postToolUse` hooks.
///
/// Inputs: `hook_type` -- the SDK hook type string; `suffix` -- "hook" or "complete".
fn map_hook_event_text(hook_type: &str, suffix: &str) -> Option<String> {
    if hook_type.eq_ignore_ascii_case("postToolUse") {
        None
    } else {
        Some(format!("[{} {}]", hook_type, suffix))
    }
}

/// Maps Debug-tier events to display text by delegating to focused sub-helpers.
///
/// Returns `None` for variants not in the Debug tier.
fn map_debug_text(event: &SessionEventData) -> Option<String> {
    map_session_state_text(event).or_else(|| map_usage_and_system_text(event))
}

/// Maps session-state debug events to display text.
///
/// Handles: SessionResume, SessionIdle, SessionInfo, SessionModelChange,
/// SessionTruncation, PendingMessagesModified, AssistantReasoning, AssistantReasoningDelta.
fn map_session_state_text(event: &SessionEventData) -> Option<String> {
    map_session_lifecycle_text(event)
        .or_else(|| map_session_model_or_truncation_text(event))
        .or_else(|| map_reasoning_text(event))
}

fn map_session_lifecycle_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::SessionResume(_) => Some("Session resumed".to_string()),
        SessionEventData::SessionIdle(_) => Some(format_session_state(true, None, None)),
        SessionEventData::SessionInfo(d) => Some(d.message.clone()),
        SessionEventData::PendingMessagesModified(_) => {
            Some("[Pending messages updated]".to_string())
        }
        _ => None,
    }
}

fn map_session_model_or_truncation_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::SessionModelChange(d) => Some(format!("Model: {}", d.new_model)),
        SessionEventData::SessionTruncation(d) => Some(format!(
            "Truncated: {} tokens",
            d.tokens_removed_during_truncation as u32
        )),
        _ => None,
    }
}

fn map_reasoning_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::AssistantReasoning(d) => Some(d.content.clone()),
        SessionEventData::AssistantReasoningDelta(d) => Some(d.delta_content.clone()),
        _ => None,
    }
}

/// Maps usage and system debug events to display text.
///
/// Handles: AssistantUsage, ToolExecutionPartialResult, SystemMessage,
/// SessionCompactionStart, SessionCompactionComplete, SessionSnapshotRewind.
fn map_usage_and_system_text(event: &SessionEventData) -> Option<String> {
    match event {
        SessionEventData::AssistantUsage(d) => {
            let data = AssistantUsageData {
                input: d.input_tokens.unwrap_or(0.0) as u32,
                output: d.output_tokens.unwrap_or(0.0) as u32,
                cache_read: d.cache_read_tokens.map(|v| v as u32),
                cost: d.cost.filter(|&c| c > 0.0),
                cache_write_tokens: d.cache_write_tokens.unwrap_or(0.0) as u32,
            };
            Some(format_assistant_usage(
                &data,
                d.model.as_deref().unwrap_or("unknown"),
            ))
        }
        SessionEventData::ToolExecutionPartialResult(d) => Some(d.partial_output.clone()),
        SessionEventData::SystemMessage(d) => Some(format!("[System] {}", d.content)),
        SessionEventData::SessionCompactionStart(_) => Some("[Compacting...]".to_string()),
        SessionEventData::SessionCompactionComplete(d) => {
            let removed = compute_tokens_removed(d);
            Some(format!("Compacted: {} tokens", removed))
        }
        SessionEventData::SessionSnapshotRewind(_) => Some("[Rewound to snapshot]".to_string()),
        _ => None,
    }
}

/// Computes the number of tokens removed during a compaction event.
fn compute_tokens_removed(d: &copilot_sdk::SessionCompactionCompleteData) -> u32 {
    if let (Some(pre), Some(post)) = (d.pre_compaction_tokens, d.post_compaction_tokens) {
        (pre - post) as u32
    } else {
        0
    }
}

/// Determines if an event should be emitted based on mode and priority.
///
/// This helper function implements the mode-based filtering logic for background events.
/// It checks whether the given event [`BackgroundEventPriority`] is included in the active
/// [`BackgroundPanelMode`]:
///
/// - `Critical` mode: only allows `Critical` priority events
/// - `Normal` mode: allows `Critical` and `Informational` priority events
/// - `Debug` mode: allows all priority events (`Critical`, `Informational`, `Debug`)
///
/// # Arguments
///
/// * `mode` - The active [`BackgroundPanelMode`] determining which tiers are visible
/// * `priority` - The [`BackgroundEventPriority`] tier of the event to check
///
/// # Returns
///
/// `true` if the event's priority tier is enabled in the current mode, `false` otherwise.
fn should_emit(mode: BackgroundPanelMode, priority: BackgroundEventPriority) -> IsPredicate {
    IsPredicate(mode.includes(priority).0)
}

/// Formats a tool invocation with extracted, human-readable arguments.
///
/// Inspects the tool name and JSON arguments to produce a short, readable line:
/// - `view`, `edit`, `create`: extracts `"path"` and strips the repo-root prefix
/// - `bash`: extracts `"command"` and truncates to 60 characters
/// - anything else: `"Tool: {name}"` with no argument detail
///
/// # Examples
///
/// ```ignore
/// let args = serde_json::json!({"path": "/home/user/repo/src/lib.rs"});
/// let line = format_tool_with_args("view", Some(&args));
/// // Output: "Tool: view → src/lib.rs"
/// ```
fn format_tool_with_args(tool_name: &str, arguments: Option<&serde_json::Value>) -> String {
    // Repo root is baked at compile time via build.rs (WORKSPACE_ROOT env var).
    // Used to strip the absolute prefix from tool paths for shorter display.
    const REPO_ROOT: &str = env!("WORKSPACE_ROOT");
    const CMD_LIMIT: usize = 60;

    fn shorten_path(path: &str) -> &str {
        if let Some(rel) = path.strip_prefix(REPO_ROOT) {
            rel
        } else {
            let char_count = path.chars().count();
            if char_count > CMD_LIMIT {
                let byte_start = path
                    .char_indices()
                    .nth(char_count - CMD_LIMIT)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                &path[byte_start..]
            } else {
                path
            }
        }
    }

    match (tool_name, arguments) {
        ("view" | "edit" | "create", Some(args)) => {
            if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
                format!("Tool: {} → {}", tool_name, shorten_path(path))
            } else {
                format!("Tool: {}", tool_name)
            }
        }
        ("bash", Some(args)) => {
            if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                let display = if cmd.chars().count() > CMD_LIMIT {
                    let byte_end = cmd
                        .char_indices()
                        .nth(CMD_LIMIT)
                        .map(|(i, _)| i)
                        .unwrap_or(cmd.len());
                    &cmd[..byte_end]
                } else {
                    cmd
                };
                format!("Tool: bash → {}", display)
            } else {
                format!("Tool: {}", tool_name)
            }
        }
        _ => format!("Tool: {}", tool_name),
    }
}

/// Formats a tool event into a concise status line.
///
/// Translates tool execution events into user-friendly status updates.
/// If execution result data is provided, reports success or failure with the tool call ID.
/// Otherwise, reports the tool name.
///
/// # Arguments
///
/// * `tool_name` - The name of the tool (used if no result data)
/// * `_args` - Optional arguments (currently unused for display)
/// * `result` - Optional tool execution result data
///
/// # Examples
///
/// Successful tool execution:
/// ```ignore
/// let status = format_tool_event("grep", None, Some(&success_result));
/// // Output: "Tool call_123 completed"
/// ```
fn format_tool_event(
    tool_name: &str,
    _args: Option<&str>,
    result: Option<&copilot_sdk::ToolExecutionCompleteData>,
) -> String {
    if let Some(res) = result {
        if res.success {
            format!("Tool {} completed", res.tool_call_id)
        } else {
            format!("Tool {} failed", res.tool_call_id)
        }
    } else {
        format!("Tool {}", tool_name)
    }
}

/// Formats session state changes into a status line.
///
/// Generates appropriate status messages based on session idle state and token usage.
/// Used to display context window information and session lifecycle events.
///
/// # Arguments
///
/// * `idle` - Whether the session is currently idle
/// * `current` - Current token usage (if available)
/// * `limit` - Token limit for the session context (if available)
///
/// # Returns
///
/// A string describing the session state:
/// - If idle: "Session idle"
/// - If token data available: "Session: X/Y tokens"
/// - Otherwise: "Session state changed"
fn format_session_state(idle: bool, current: Option<u32>, limit: Option<u32>) -> String {
    if idle {
        "Session idle".to_string()
    } else if let (Some(c), Some(l)) = (current, limit) {
        format!("Session: {}/{} tokens", c, l)
    } else {
        "Session state changed".to_string()
    }
}

/// Holds assistant usage metrics.
struct AssistantUsageData {
    input: u32,
    output: u32,
    cache_read: Option<u32>,
    cost: Option<f64>,
    cache_write_tokens: u32,
}

/// Formats assistant usage metrics into a concise display.
fn format_assistant_usage(data: &AssistantUsageData, model: &str) -> String {
    let mut result = if let Some(cache) = data.cache_read {
        format!(
            "{}: in={} out={} cache={}",
            model, data.input, data.output, cache
        )
    } else {
        format!("{}: in={} out={}", model, data.input, data.output)
    };

    if let Some(cost) = data.cost {
        result.push_str(&format!(" | ${:.2}", cost));
    }

    if data.cache_write_tokens > 0 {
        result.push_str(&format!(" | writes {}k", data.cache_write_tokens / 1000));
    }

    result
}

/// Formats the assistant's current intent into a one-line summary.
fn format_intent(text: &str) -> String {
    format!("Intent: {}", text)
}

/// Truncates text to the specified character limit, appending "..." if truncated.
///
/// Ensures that event text respects tier-specific character limits by truncating
/// and appending an ellipsis ("...") when the text exceeds the limit. The truncation
/// accounts for the 3-character ellipsis, so the total output is exactly `limit` characters.
///
/// # Arguments
///
/// * `text` - The text to truncate
/// * `limit` - The maximum character length of the output
///
/// # Returns
///
/// The truncated text (if longer than `limit`) or the original text (if within limit).
/// Truncated output always ends with "..." and is exactly `limit` characters long.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(truncate_to_limit("hello", 10), "hello");
/// assert_eq!(truncate_to_limit("hello world", 8), "hello...");
/// assert_eq!(truncate_to_limit("x".repeat(200), 50).len(), 50);
/// ```
fn truncate_to_limit(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        text.to_string()
    } else {
        let truncate_at = limit.saturating_sub(3);
        let safe_end = floor_char_boundary(text, truncate_at);
        format!("{}...", &text[..safe_end])
    }
}

/// Returns the largest char boundary index that is ≤ `index` within `s`.
///
/// Walks backward from `index` until `s.is_char_boundary` is true, ensuring
/// byte-slice operations never land mid-codepoint.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let clamped = index.min(s.len());
    let mut i = clamped;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Extract structured `LlmUsage` from a `SessionEventData::AssistantUsage` event.
///
/// Returns `None` for all other event variants. The `temperature` field is not
/// available in the SDK usage event and defaults to zero.
///
/// # Postconditions
///
/// - Returns `Some` iff `event` is `AssistantUsage`.
/// - All token counts are non-negative (SDK `f64` fields are floored at 0.0 before cast).
/// - `cost_usd` is `0.0` when the SDK omits the cost field.
pub(crate) fn extract_llm_usage(event: &SessionEventData) -> Option<LlmUsage> {
    match event {
        SessionEventData::AssistantUsage(d) => Some(LlmUsage {
            model: OutputText::new(d.model.as_deref().unwrap_or("unknown")),
            token_counts: LlmTokenCounts {
                tokens_in: TokenCount::new(d.input_tokens.unwrap_or(0.0) as u64),
                tokens_out: TokenCount::new(d.output_tokens.unwrap_or(0.0) as u64),
                tokens_cached: TokenCount::new(d.cache_read_tokens.unwrap_or(0.0) as u64),
                cache_write_tokens: TokenCount::new(d.cache_write_tokens.unwrap_or(0.0) as u64),
                cost_usd: d.cost.unwrap_or(0.0).max(0.0).into(),
            },
            temperature: Default::default(),
        }),
        _ => None,
    }
}

/// Map a `SessionEventData` to a [`BackgroundMappedEvent`] carrying both display and usage.
///
/// Combines [`map_background_event`] (for the display side) with [`extract_llm_usage`]
/// (for the usage side) in a single call so callers can forward usage to the
/// token-tracker actor without traversing the event twice.
///
/// # Postconditions
///
/// - `result.display` is identical to what `map_background_event(event, priority, mode)`
///   would return.
/// - `result.usage.is_some()` iff `event` is `SessionEventData::AssistantUsage`.
pub(crate) fn map_background_event_with_usage(
    event: &SessionEventData,
    priority: BackgroundEventPriority,
    mode: BackgroundPanelMode,
) -> BackgroundMappedEvent {
    BackgroundMappedEvent {
        display: map_background_event(event, priority, mode),
        usage: extract_llm_usage(event),
    }
}
