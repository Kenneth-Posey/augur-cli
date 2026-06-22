//! Pure mapping from `copilot_sdk::SessionEventData` to `AgentOutput`.
//!
//! Contains no I/O and no actor state. The actor passes each SDK event
//! directly to `map_sdk_event`; the dispatch loop forwards the result when
//! `Some`. Gated on `copilot-executor` because it uses SDK types.

use augur_domain::ExecutionSuccess;
use augur_domain::string_newtypes::{ModelId, OutputText, StringNewtype, ToolCallId, ToolName};
use augur_domain::types::AgentOutput;

/// Map an SDK session event to an `AgentOutput`, if one applies.
///
/// Returns `Some(output)` for events that have a direct representation in
/// the agent output stream. Returns `None` for informational or lifecycle
/// events that require no TUI action (e.g., `SessionStart`, `SessionResume`).
///
/// Suppression rules for background-agent routing are applied upstream by
/// `FeedRouter::compute_main_out`; this function performs only structural
/// event mapping.
///
/// Mapping rules:
/// - `AssistantMessageDelta` → `Token` (streaming text chunk).
/// - `AssistantMessage` without tool requests → `Done` (signals end of assistant output).
/// - `AssistantMessage` with tool requests → `MessageBreak` (preserves turn activity while
///   tools execute and the loop continues).
/// - `SessionIdle` → `TurnComplete` (turn is fully idle and ready for next).
/// - `SessionError` → `Error`.
/// - `Abort` → `Error` with the abort reason.
/// - `AssistantUsage` → `UsageUpdate` with model name.
/// - `ToolExecutionStart` → `ToolCallStarted`.
/// - `ToolExecutionComplete` → `ToolCallCompleted`.
///   `result` is the success content when available, or the error message from
///   `error.message` when the tool failed and `result` is absent.
/// - `AssistantIntent` → `IntentMessage` with the model's stated intent text.
/// - `ToolExecutionProgress` → `ToolProgress`.
/// - `ToolExecutionPartialResult` → `ToolPartialResult`.
/// - `SessionCompactionStart` → `SystemMessage` with "\[system\] compacting context..." so
///   the user sees a timestamped indicator when compaction fires.
/// - `SessionCompactionComplete` → `CompactionComplete` on success (human-readable
///   summary + `post_tokens` for immediate status bar update), or `Error` on failure.
/// - Everything else → `None`.
///
/// Called by `FeedRouter::compute_main_out` for every event received from the
/// Copilot CLI session. The result is forwarded on the broadcast channel when `Some`.
pub fn map_sdk_event(event: &copilot_sdk::SessionEventData) -> Option<AgentOutput> {
    map_event_to_output(event)
}

/// Dispatch an SDK event to the appropriate `AgentOutput` variant.
///
/// Contains the 14-arm match over `SessionEventData`. Suppression policy is
/// handled upstream by `map_sdk_event`; this function only performs the
/// structural mapping. Tool, usage, and compaction event groups are delegated
/// to focused sub-helpers to keep each function within complexity limits.
///
/// Returns `None` for variants that have no output representation (e.g.,
/// `SessionStart`, or any future unknown variants).
/// Map an `AssistantMessageDelta` content string to a `Token` output.
///
/// Returns `None` when `content` is empty (no delta to display).
fn map_assistant_delta_output(content: &str) -> Option<AgentOutput> {
    (!content.is_empty()).then(|| AgentOutput::Token(OutputText::new(content.to_owned())))
}

/// Map a Copilot SDK event into the main conversation output stream.
///
/// Returns `Some(AgentOutput)` for events that should be rendered in the
/// primary feed, or `None` when the event has no main-feed representation.
pub(crate) fn map_event_to_output(event: &copilot_sdk::SessionEventData) -> Option<AgentOutput> {
    map_primary_event(event)
        .or_else(|| map_intent_or_abort_event(event))
        .or_else(|| map_tool_event(event))
        .or_else(|| map_usage_event(event))
        .or_else(|| map_compaction_event(event))
}

fn map_primary_event(event: &copilot_sdk::SessionEventData) -> Option<AgentOutput> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::AssistantMessageDelta(d) => map_assistant_delta_output(&d.delta_content),
        E::AssistantMessage(d) => Some(map_assistant_message_output(d)),
        E::SessionIdle(_) => Some(AgentOutput::TurnComplete),
        E::SessionError(d) => map_session_error(d),
        _ => None,
    }
}

fn map_assistant_message_output(d: &copilot_sdk::AssistantMessageData) -> AgentOutput {
    if d.tool_requests.is_some() {
        AgentOutput::MessageBreak
    } else {
        AgentOutput::Done
    }
}

fn map_intent_or_abort_event(event: &copilot_sdk::SessionEventData) -> Option<AgentOutput> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::Abort(d) => Some(AgentOutput::Error(OutputText::new(d.reason.clone()))),
        E::AssistantIntent(d) => Some(AgentOutput::IntentMessage(OutputText::new(
            d.intent.clone(),
        ))),
        _ => None,
    }
}

/// Map tool execution events to `AgentOutput`.
///
/// Handles `ToolExecutionStart`, `ToolExecutionComplete`, `ToolExecutionProgress`,
/// and `ToolExecutionPartialResult`. Called from `map_event_to_output` for the
/// combined tool arm. Returns `None` for any non-tool variant (unreachable in
/// practice but required for match exhaustiveness).
fn map_tool_event(event: &copilot_sdk::SessionEventData) -> Option<AgentOutput> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::ToolExecutionStart(d) => {
            let args = d.arguments.clone().unwrap_or(serde_json::Value::Null);
            Some(AgentOutput::ToolCallStarted {
                name: ToolName::new(d.tool_name.clone()),
                args,
            })
        }
        E::ToolExecutionComplete(d) => {
            let result = d
                .result
                .as_ref()
                .map(|r| OutputText::new(r.content.clone()))
                .or_else(|| d.error.as_ref().map(|e| OutputText::new(e.message.clone())));
            Some(AgentOutput::ToolCallCompleted {
                name: ToolName::new(d.tool_call_id.clone()),
                success: ExecutionSuccess::from(d.success),
                result,
                session_log: None,
            })
        }
        E::ToolExecutionProgress(d) => Some(AgentOutput::ToolProgress {
            tool_call_id: ToolCallId::from(d.tool_call_id.as_str()),
            message: OutputText::new(d.progress_message.clone()),
        }),
        E::ToolExecutionPartialResult(d) => Some(AgentOutput::ToolPartialResult {
            tool_call_id: ToolCallId::from(d.tool_call_id.as_str()),
            output: OutputText::new(d.partial_output.clone()),
        }),
        _ => None,
    }
}

/// Map usage events to `AgentOutput`.
///
/// Handles `AssistantUsage` (model name per turn). Called from `map_event_to_output`
/// for the usage arm. Returns `None` for any non-usage variant (unreachable in practice).
fn map_usage_event(event: &copilot_sdk::SessionEventData) -> Option<AgentOutput> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::AssistantUsage(d) => Some(AgentOutput::UsageUpdate {
            model: d.model.as_deref().map(ModelId::from),
        }),
        _ => None,
    }
}

/// Map compaction lifecycle events to `AgentOutput`.
///
/// Handles `SessionCompactionStart` (emits a "[system] compacting context..."
/// indicator) and `SessionCompactionComplete` (delegates to
/// `format_compaction_complete` for success/failure formatting). Called from
/// `map_event_to_output` for the combined compaction arm. Returns `None` for
/// any non-compaction variant (unreachable in practice).
fn map_compaction_event(event: &copilot_sdk::SessionEventData) -> Option<AgentOutput> {
    use copilot_sdk::SessionEventData as E;
    match event {
        E::SessionCompactionStart(_) => Some(AgentOutput::SystemMessage(OutputText::new(
            "[system] compacting context...".to_owned(),
        ))),
        E::SessionCompactionComplete(d) => Some(format_compaction_complete(d)),
        _ => None,
    }
}

/// Map a `SessionError` event to `AgentOutput::Error`.
///
/// Forwards all session errors as `AgentOutput::Error` so they appear in the
/// conversation flow. Called from `map_sdk_event` for the `SessionError` arm.
fn map_session_error(d: &copilot_sdk::SessionErrorData) -> Option<AgentOutput> {
    Some(AgentOutput::Error(OutputText::new(d.message.clone())))
}

/// Build an `AgentOutput` from a `SessionCompactionCompleteData` payload.
///
/// On success, formats a human-readable summary and packages the result as
/// `AgentOutput::CompactionComplete`.
///
/// On failure, wraps the error message in `AgentOutput::Error`.
///
/// Consumers: `map_sdk_event` for the `SessionCompactionComplete` arm.
fn format_compaction_complete(d: &copilot_sdk::SessionCompactionCompleteData) -> AgentOutput {
    if !d.success {
        let msg = d
            .error
            .clone()
            .unwrap_or_else(|| "compaction failed".to_owned());
        return AgentOutput::Error(OutputText::new(msg));
    }
    let text = match (d.pre_compaction_tokens, d.post_compaction_tokens) {
        (Some(pre), Some(post)) => format!(
            "[system] context compacted: {} \u{2192} {} tokens",
            pre as u64, post as u64,
        ),
        _ => "[system] context compacted".to_owned(),
    };
    AgentOutput::CompactionComplete {
        text: OutputText::new(text),
    }
}
