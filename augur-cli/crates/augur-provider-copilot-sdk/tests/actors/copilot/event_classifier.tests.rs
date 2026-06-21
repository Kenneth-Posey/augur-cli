use augur_domain::background_events::BackgroundEventClassifier;
use augur_domain::background_events::*;
use augur_domain::newtypes::{BufferThreshold, ErrorMessage, ExecutionSuccess, TimestampMs};
use augur_domain::string_newtypes::{ContentDelta, StringNewtype, ToolName};
use augur_provider_copilot_sdk::actors::copilot::event_classifier::CopilotEventClassifier;
use copilot_sdk::events::{
    AbortData, AssistantIntentData, AssistantMessageData, AssistantMessageDeltaData,
    AssistantReasoningData, AssistantReasoningDeltaData, AssistantTurnEndData,
    AssistantTurnStartData, CustomAgentCompletedData, CustomAgentFailedData,
    CustomAgentSelectedData, CustomAgentStartedData, ExternalToolRequestedData, HandoffSourceType,
    HookEndData, HookStartData, PermissionRequestedData, SessionErrorData, SessionHandoffData,
    SessionIdleData, SessionInfoData, SessionModelChangeData, SessionResumeData,
    SessionShutdownData, SessionSnapshotRewindData, SessionStartData, SessionTruncationData,
    SessionUsageInfoData, ShutdownCodeChanges, ShutdownType, SkillInvokedData,
    SystemMessageEventData, SystemMessageRole, ToolExecutionCompleteData,
    ToolExecutionPartialResultData, ToolExecutionProgressData, ToolExecutionStartData,
    ToolUserRequestedData, UserMessageData,
};
use copilot_sdk::SessionEventData;

use std::collections::HashMap;

fn classify_event(event: &SessionEventData) -> Option<BackgroundEventPriority> {
    let classifier = CopilotEventClassifier;
    classifier.classify(event)
}

// ============================================================================
// BackgroundEventPriority: 6 tests
// ============================================================================

/// BackgroundEventPriority::is_critical() returns true only for Critical tier.
#[test]
fn test_priority_is_critical_true_for_critical() {
    let priority = BackgroundEventPriority::Critical;
    assert!(priority.is_critical().0);
}

/// BackgroundEventPriority::is_critical() returns false for non-Critical tiers.
#[test]
fn test_priority_is_critical_false_for_other_tiers() {
    assert!(!BackgroundEventPriority::Informational.is_critical().0);
    assert!(!BackgroundEventPriority::Debug.is_critical().0);
}

/// BackgroundEventPriority::is_informational() returns true only for Informational tier.
#[test]
fn test_priority_is_informational_true_for_informational() {
    let priority = BackgroundEventPriority::Informational;
    assert!(priority.is_informational().0);
}

/// BackgroundEventPriority::is_informational() returns false for non-Informational tiers.
#[test]
fn test_priority_is_informational_false_for_other_tiers() {
    assert!(!BackgroundEventPriority::Critical.is_informational().0);
    assert!(!BackgroundEventPriority::Debug.is_informational().0);
}

/// BackgroundEventPriority::is_debug() returns true only for Debug tier.
#[test]
fn test_priority_is_debug_true_for_debug() {
    let priority = BackgroundEventPriority::Debug;
    assert!(priority.is_debug().0);
}

/// BackgroundEventPriority::is_debug() returns false for non-Debug tiers.
#[test]
fn test_priority_is_debug_false_for_other_tiers() {
    assert!(!BackgroundEventPriority::Critical.is_debug().0);
    assert!(!BackgroundEventPriority::Informational.is_debug().0);
}

// ============================================================================
// BackgroundPanelMode: 5 tests
// ============================================================================

/// BackgroundPanelMode::Critical includes only Critical priority events.
#[test]
fn test_mode_critical_includes_only_critical_priority() {
    let mode = BackgroundPanelMode::Critical;
    assert!(mode.includes(BackgroundEventPriority::Critical).0);
    assert!(!mode.includes(BackgroundEventPriority::Informational).0);
    assert!(!mode.includes(BackgroundEventPriority::Debug).0);
}

/// BackgroundPanelMode::Normal includes Critical and Informational, filters Debug.
#[test]
fn test_mode_normal_includes_critical_and_informational() {
    let mode = BackgroundPanelMode::Normal;
    assert!(mode.includes(BackgroundEventPriority::Critical).0);
    assert!(mode.includes(BackgroundEventPriority::Informational).0);
    assert!(!mode.includes(BackgroundEventPriority::Debug).0);
}

/// BackgroundPanelMode::Debug includes all priority tiers.
#[test]
fn test_mode_debug_includes_all_priorities() {
    let mode = BackgroundPanelMode::Debug;
    assert!(mode.includes(BackgroundEventPriority::Critical).0);
    assert!(mode.includes(BackgroundEventPriority::Informational).0);
    assert!(mode.includes(BackgroundEventPriority::Debug).0);
}

/// BackgroundPanelMode::label() returns correct display label for Critical mode.
#[test]
fn test_mode_label_critical() {
    assert_eq!(BackgroundPanelMode::Critical.label().as_str(), "Critical");
}

/// BackgroundPanelMode::label() returns correct display labels for Normal and Debug modes.
#[test]
fn test_mode_label_normal_and_debug() {
    assert_eq!(BackgroundPanelMode::Normal.label().as_str(), "Normal");
    assert_eq!(BackgroundPanelMode::Debug.label().as_str(), "Debug");
}

// ============================================================================
// DeltaAccumulator: 7 tests
// ============================================================================

/// DeltaAccumulator::push() returns None when accumulated content stays under threshold.
#[test]
fn test_delta_push_under_threshold_returns_none() {
    let mut acc = DeltaAccumulator::default();
    let result = acc.push(ContentDelta::new("small"), BufferThreshold(200));
    assert_eq!(result, None);
}

/// DeltaAccumulator::push() returns Some when accumulated content exceeds threshold.
#[test]
fn test_delta_push_over_threshold_returns_flushed() {
    let mut acc = DeltaAccumulator::default();
    let delta1 = "x".repeat(100);
    let delta2 = "y".repeat(120);

    assert_eq!(
        acc.push(ContentDelta::new(&delta1), BufferThreshold(150)),
        None
    );
    let flushed = acc.push(ContentDelta::new(&delta2), BufferThreshold(150));

    assert!(flushed.is_some());
    let content = flushed.unwrap();
    assert!(content.as_str().contains("x"));
    assert!(content.as_str().contains("y"));
}

/// DeltaAccumulator::flush() returns accumulated content on first call.
#[test]
fn test_delta_flush_returns_content() {
    let mut acc = DeltaAccumulator::default();
    acc.push(ContentDelta::new("content"), BufferThreshold(500));

    let flushed = acc.flush();
    assert_eq!(
        flushed.map(|c| c.as_str().to_string()),
        Some("content".to_string())
    );
}

/// DeltaAccumulator::flush() returns None on empty buffer.
#[test]
fn test_delta_flush_empty_returns_none() {
    let mut acc = DeltaAccumulator::default();
    assert_eq!(acc.flush(), None);
}

/// DeltaAccumulator::flush() is idempotent; second call returns None.
#[test]
fn test_delta_flush_idempotent() {
    let mut acc = DeltaAccumulator::default();
    acc.push(ContentDelta::new("data"), BufferThreshold(500));

    let first = acc.flush();
    let second = acc.flush();

    assert!(first.is_some());
    assert_eq!(second, None);
}

/// DeltaAccumulator::peek() returns content reference without flushing.
#[test]
fn test_delta_peek_returns_ref_without_flush() {
    let mut acc = DeltaAccumulator::default();
    acc.push(ContentDelta::new("inspect"), BufferThreshold(500));

    let peeked = acc.peek();
    assert!(peeked.is_some());
    assert_eq!(peeked.unwrap().as_str(), "inspect");

    // Content still available after peek
    let peeked_again = acc.peek();
    assert!(peeked_again.is_some());
    assert_eq!(peeked_again.unwrap().as_str(), "inspect");
}

/// DeltaAccumulator::peek() returns None for empty buffer.
#[test]
fn test_delta_peek_empty_returns_none() {
    let acc = DeltaAccumulator::default();
    assert_eq!(acc.peek(), None);
}

// ============================================================================
// ToolExecutionMetadata: 2 tests
// ============================================================================

/// ToolExecutionMetadata::new() constructs with all fields accessible.
#[test]
fn test_metadata_new_stores_fields() {
    let tool_name = ToolName::from("my_tool");
    let tool_args = serde_json::json!({"key": "value"});
    let started_at_ms = TimestampMs::from(1234567890u64);

    let meta = ToolExecutionMetadata::new(tool_name.clone(), tool_args.clone(), started_at_ms);

    assert_eq!(meta.tool_name, tool_name);
    assert_eq!(meta.tool_args, tool_args);
    assert_eq!(meta.started_at_ms, started_at_ms);
}

/// ToolExecutionMetadata is serializable and deserializable with serde.
#[test]
fn test_metadata_serde_roundtrip() {
    let tool_name = ToolName::from("serde_tool");
    let tool_args = serde_json::json!({"arg1": "val1", "arg2": 42});
    let started_at_ms = TimestampMs::from(9876543210u64);

    let original = ToolExecutionMetadata::new(tool_name, tool_args, started_at_ms);

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: ToolExecutionMetadata = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(restored.tool_name, original.tool_name);
    assert_eq!(restored.tool_args, original.tool_args);
    assert_eq!(restored.started_at_ms, original.started_at_ms);
}

// ============================================================================
// ToolExecutionResult: 3 tests
// ============================================================================

/// ToolExecutionResult::new() constructs successful result with empty progress.
#[test]
fn test_result_new_success() {
    let result = ToolExecutionResult::new(ExecutionSuccess::success(), None);

    assert!(result.success.0);
    assert_eq!(result.error, None);
    assert!(result.progress_messages.is_empty());
}

/// ToolExecutionResult::new() constructs failure result with error message.
#[test]
fn test_result_new_failure() {
    let error = Some(ErrorMessage::new("timeout"));
    let result = ToolExecutionResult::new(ExecutionSuccess::failure(), error.clone());

    assert!(!result.success.0);
    assert_eq!(result.error, error);
    assert!(result.progress_messages.is_empty());
}

/// ToolExecutionResult::to_display_line() formats success/failure appropriately.
#[test]
fn test_result_to_display_line_formatting() {
    // Success case
    let success = ToolExecutionResult::new(ExecutionSuccess::success(), None);
    let success_line = success.to_display_line(ToolName::new("success_tool"));
    assert!(success_line.contains("✓"));
    assert!(success_line.contains("success_tool"));
    assert!(success_line.contains("completed"));

    // Failure case with error
    let error_msg = "network unreachable";
    let failure = ToolExecutionResult::new(
        ExecutionSuccess::failure(),
        Some(ErrorMessage::new(error_msg)),
    );
    let failure_line = failure.to_display_line(ToolName::new("failure_tool"));
    assert!(failure_line.contains("✗"));
    assert!(failure_line.contains("failure_tool"));
    assert!(failure_line.contains("failed"));
    assert!(failure_line.contains(error_msg));
}

// ============================================================================
// classify_event() Test Helpers: 40 SessionEventData creators
// ============================================================================

fn test_session_start() -> SessionStartData {
    SessionStartData {
        session_id: "test".to_string(),
        version: 1.0,
        producer: "test".to_string(),
        copilot_version: "1.0".to_string(),
        start_time: "2024-01-01".to_string(),
        selected_model: None,
    }
}

fn test_session_error() -> SessionErrorData {
    SessionErrorData {
        error_type: "test_error".to_string(),
        message: "test error message".to_string(),
        stack: None,
        code: None,
        provider_call_id: None,
    }
}

fn test_session_shutdown() -> SessionShutdownData {
    SessionShutdownData {
        shutdown_type: ShutdownType::Routine,
        error_reason: None,
        total_premium_requests: 0.0,
        total_api_duration_ms: 0.0,
        session_start_time: 0.0,
        code_changes: ShutdownCodeChanges::default(),
        model_metrics: HashMap::new(),
        current_model: None,
    }
}

fn test_user_message() -> UserMessageData {
    UserMessageData {
        content: "test message".to_string(),
        transformed_content: None,
        attachments: None,
        source: None,
    }
}

fn test_session_info() -> SessionInfoData {
    SessionInfoData {
        info_type: "info".to_string(),
        message: "test info message".to_string(),
    }
}

fn test_permission_requested() -> PermissionRequestedData {
    PermissionRequestedData {
        request_id: None,
        permission_request: None,
    }
}

fn test_external_tool_requested() -> ExternalToolRequestedData {
    ExternalToolRequestedData {
        request_id: None,
        tool_name: Some("test_tool".to_string()),
        tool_call_id: Some("call_123".to_string()),
        arguments: None,
    }
}

fn test_system_message() -> SystemMessageEventData {
    SystemMessageEventData {
        content: "test".to_string(),
        role: SystemMessageRole::System,
        name: None,
        metadata: None,
    }
}

fn test_abort() -> AbortData {
    AbortData {
        reason: "test".to_string(),
    }
}

fn test_custom_agent_failed() -> CustomAgentFailedData {
    CustomAgentFailedData {
        tool_call_id: "call_123".to_string(),
        agent_name: "test_agent".to_string(),
        error: "test error".to_string(),
    }
}

fn test_assistant_turn_start() -> AssistantTurnStartData {
    AssistantTurnStartData {
        turn_id: "turn_1".to_string(),
    }
}

fn test_assistant_intent() -> AssistantIntentData {
    AssistantIntentData {
        intent: "test intent".to_string(),
    }
}

fn test_assistant_reasoning() -> AssistantReasoningData {
    AssistantReasoningData {
        reasoning_id: "reasoning_1".to_string(),
        content: "Thinking about this problem carefully".to_string(),
        chunk_content: None,
    }
}

fn test_assistant_reasoning_delta() -> AssistantReasoningDeltaData {
    AssistantReasoningDeltaData {
        reasoning_id: "reasoning_1".to_string(),
        delta_content: "incremental reasoning update".to_string(),
    }
}

fn test_assistant_message() -> AssistantMessageData {
    AssistantMessageData {
        message_id: "msg_1".to_string(),
        content: "Here's my answer to your question".to_string(),
        chunk_content: None,
        total_response_size_bytes: None,
        tool_requests: None,
        parent_tool_call_id: None,
    }
}

fn test_assistant_message_delta() -> AssistantMessageDeltaData {
    AssistantMessageDeltaData {
        message_id: "msg_1".to_string(),
        delta_content: "partial response chunk".to_string(),
        parent_tool_call_id: None,
        total_response_size_bytes: None,
    }
}

fn test_assistant_turn_end() -> AssistantTurnEndData {
    AssistantTurnEndData {
        turn_id: "turn_1".to_string(),
    }
}

fn test_assistant_usage() -> copilot_sdk::AssistantUsageData {
    copilot_sdk::AssistantUsageData {
        model: Some("gpt-4".to_string()),
        input_tokens: Some(100.0),
        output_tokens: Some(50.0),
        cache_read_tokens: None,
        cache_write_tokens: None,
        cost: None,
        duration: None,
        initiator: None,
        api_call_id: None,
        provider_call_id: None,
        quota_snapshots: None,
    }
}

fn test_tool_user_requested() -> ToolUserRequestedData {
    ToolUserRequestedData {
        tool_call_id: "call_1".to_string(),
        tool_name: "test_tool".to_string(),
        arguments: None,
    }
}

fn test_tool_execution_start() -> ToolExecutionStartData {
    ToolExecutionStartData {
        tool_call_id: "call_1".to_string(),
        tool_name: "test_tool".to_string(),
        arguments: None,
        parent_tool_call_id: None,
    }
}

fn test_tool_execution_complete() -> ToolExecutionCompleteData {
    ToolExecutionCompleteData {
        tool_call_id: "call_1".to_string(),
        success: true,
        is_user_requested: None,
        result: None,
        error: None,
        tool_telemetry: None,
        parent_tool_call_id: None,
        mcp_server_name: None,
        mcp_tool_name: None,
    }
}

fn test_tool_execution_progress() -> ToolExecutionProgressData {
    ToolExecutionProgressData {
        tool_call_id: "call_1".to_string(),
        progress_message: "test progress".to_string(),
    }
}

fn test_tool_execution_partial_result() -> ToolExecutionPartialResultData {
    ToolExecutionPartialResultData {
        tool_call_id: "call_1".to_string(),
        partial_output: "test".to_string(),
    }
}

fn test_custom_agent_started() -> CustomAgentStartedData {
    CustomAgentStartedData {
        tool_call_id: "call_1".to_string(),
        agent_name: "test_agent".to_string(),
        agent_display_name: "Test Agent".to_string(),
        agent_description: "Test description".to_string(),
    }
}

fn test_custom_agent_completed() -> CustomAgentCompletedData {
    CustomAgentCompletedData {
        tool_call_id: "call_1".to_string(),
        agent_name: "test_agent".to_string(),
    }
}

fn test_custom_agent_selected() -> CustomAgentSelectedData {
    CustomAgentSelectedData {
        agent_name: "test_agent".to_string(),
        agent_display_name: "Test Agent".to_string(),
        tools: vec![],
    }
}

fn test_hook_start() -> HookStartData {
    HookStartData {
        hook_invocation_id: "hook_1".to_string(),
        hook_type: "test_hook".to_string(),
        input: None,
    }
}

fn test_hook_end() -> HookEndData {
    HookEndData {
        hook_invocation_id: "hook_1".to_string(),
        hook_type: "test_hook".to_string(),
        output: None,
        success: true,
        error: None,
    }
}

fn test_skill_invoked() -> SkillInvokedData {
    SkillInvokedData {
        name: "test_skill".to_string(),
        path: "/test".to_string(),
        content: "test".to_string(),
        allowed_tools: None,
    }
}

fn test_pending_messages_modified() -> copilot_sdk::PendingMessagesModifiedData {
    copilot_sdk::PendingMessagesModifiedData {}
}

fn test_session_compaction_start() -> copilot_sdk::SessionCompactionStartData {
    copilot_sdk::SessionCompactionStartData {}
}

fn test_session_compaction_complete() -> copilot_sdk::SessionCompactionCompleteData {
    copilot_sdk::SessionCompactionCompleteData {
        success: true,
        error: None,
        pre_compaction_tokens: Some(1000.0),
        post_compaction_tokens: Some(800.0),
        pre_compaction_messages_length: None,
        post_compaction_messages_length: None,
        compaction_tokens_used: None,
        messages_removed: None,
        tokens_removed: Some(200.0),
        summary_content: None,
        checkpoint_number: None,
        checkpoint_path: None,
    }
}

fn test_session_handoff() -> SessionHandoffData {
    SessionHandoffData {
        handoff_time: "2024-01-01".to_string(),
        source_type: HandoffSourceType::Local,
        repository: None,
        context: None,
        summary: None,
        remote_session_id: None,
    }
}

fn test_session_resume() -> SessionResumeData {
    SessionResumeData {
        resume_time: "1000".to_string(),
        event_count: 5.0,
    }
}

fn test_session_idle() -> SessionIdleData {
    SessionIdleData::default()
}

fn test_session_model_change() -> SessionModelChangeData {
    SessionModelChangeData {
        previous_model: None,
        new_model: "gpt-4".to_string(),
    }
}

fn test_session_truncation() -> SessionTruncationData {
    SessionTruncationData {
        token_limit: 2000.0,
        pre_truncation_tokens_in_messages: 1000.0,
        pre_truncation_messages_length: 10.0,
        post_truncation_tokens_in_messages: 500.0,
        post_truncation_messages_length: 5.0,
        tokens_removed_during_truncation: 500.0,
        messages_removed_during_truncation: 5.0,
        performed_by: "system".to_string(),
    }
}

fn test_session_snapshot_rewind() -> SessionSnapshotRewindData {
    SessionSnapshotRewindData {
        up_to_event_id: "event_123".to_string(),
        events_removed: 10.0,
    }
}

fn test_session_usage_info() -> SessionUsageInfoData {
    SessionUsageInfoData {
        token_limit: 2000.0,
        current_tokens: 1000.0,
        messages_length: 10.0,
    }
}

// ============================================================================
// CRITICAL TIER TESTS: 6 tests
// ============================================================================

/// Session lifecycle events that initialize sessions are Critical (require immediate persistence).
#[test]
fn test_classify_session_start_returns_critical() {
    let event_data = test_session_start();
    let event = SessionEventData::SessionStart(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Critical));
}

/// Session errors block normal operation and require Critical priority logging.
#[test]
fn test_classify_session_error_returns_critical() {
    let event_data = test_session_error();
    let event = SessionEventData::SessionError(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Critical));
}

/// Session shutdown is a critical lifecycle transition requiring immediate logging.
#[test]
fn test_classify_session_shutdown_returns_critical() {
    let event_data = test_session_shutdown();
    let event = SessionEventData::SessionShutdown(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Critical));
}

/// Abort signals critical termination of operation and require immediate visibility.
#[test]
fn test_classify_abort_returns_critical() {
    let event_data = test_abort();
    let event = SessionEventData::Abort(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Critical));
}

/// Agent failures are Critical failures requiring immediate attention.
#[test]
fn test_classify_custom_agent_failed_returns_critical() {
    let event_data = test_custom_agent_failed();
    let event = SessionEventData::CustomAgentFailed(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Critical));
}

/// Permission requests are user-blocking events requiring Critical priority.
#[test]
fn test_classify_permission_requested_returns_critical() {
    let event_data = test_permission_requested();
    let event = SessionEventData::PermissionRequested(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Critical));
}

// ============================================================================
// INFORMATIONAL TIER TESTS: 18 tests
// ============================================================================

/// User input messages are Informational conversation flow events.
#[test]
fn test_classify_user_message_returns_informational() {
    let event_data = test_user_message();
    let event = SessionEventData::UserMessage(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Assistant turn markers are Informational progress indicators.
#[test]
fn test_classify_assistant_turn_start_returns_informational() {
    let event_data = test_assistant_turn_start();
    let event = SessionEventData::AssistantTurnStart(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Assistant intent signals Informational reasoning state.
#[test]
fn test_classify_assistant_intent_returns_informational() {
    let event_data = test_assistant_intent();
    let event = SessionEventData::AssistantIntent(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Assistant messages are Informational content delivery.
#[test]
fn test_classify_assistant_message_returns_informational() {
    let event_data = test_assistant_message();
    let event = SessionEventData::AssistantMessage(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Message deltas are Informational incremental content.
#[test]
fn test_classify_assistant_message_delta_returns_informational() {
    let event_data = test_assistant_message_delta();
    let event = SessionEventData::AssistantMessageDelta(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Turn completion is Informational progress.
#[test]
fn test_classify_assistant_turn_end_returns_informational() {
    let event_data = test_assistant_turn_end();
    let event = SessionEventData::AssistantTurnEnd(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// User tool requests are Informational action signals.
#[test]
fn test_classify_tool_user_requested_returns_informational() {
    let event_data = test_tool_user_requested();
    let event = SessionEventData::ToolUserRequested(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Tool execution initiation is Informational progress.
#[test]
fn test_classify_tool_execution_start_returns_informational() {
    let event_data = test_tool_execution_start();
    let event = SessionEventData::ToolExecutionStart(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Tool completion events are Informational regardless of success field.
#[test]
fn test_classify_tool_execution_complete_returns_informational() {
    let event_data = test_tool_execution_complete();
    let event = SessionEventData::ToolExecutionComplete(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Tool progress updates are Informational status.
#[test]
fn test_classify_tool_execution_progress_returns_informational() {
    let event_data = test_tool_execution_progress();
    let event = SessionEventData::ToolExecutionProgress(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Agent startup is Informational lifecycle.
#[test]
fn test_classify_custom_agent_started_returns_informational() {
    let event_data = test_custom_agent_started();
    let event = SessionEventData::CustomAgentStarted(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Successful agent completion is Informational.
#[test]
fn test_classify_custom_agent_completed_returns_informational() {
    let event_data = test_custom_agent_completed();
    let event = SessionEventData::CustomAgentCompleted(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Agent selection is Informational routing.
#[test]
fn test_classify_custom_agent_selected_returns_informational() {
    let event_data = test_custom_agent_selected();
    let event = SessionEventData::CustomAgentSelected(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Hook execution start is Informational.
#[test]
fn test_classify_hook_start_returns_informational() {
    let event_data = test_hook_start();
    let event = SessionEventData::HookStart(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Hook completion is Informational.
#[test]
fn test_classify_hook_end_returns_informational() {
    let event_data = test_hook_end();
    let event = SessionEventData::HookEnd(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Skill invocation is Informational action.
#[test]
fn test_classify_skill_invoked_returns_informational() {
    let event_data = test_skill_invoked();
    let event = SessionEventData::SkillInvoked(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// External tool requests are Informational.
#[test]
fn test_classify_external_tool_requested_returns_informational() {
    let event_data = test_external_tool_requested();
    let event = SessionEventData::ExternalToolRequested(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

/// Session handoff is Informational state transition.
#[test]
fn test_classify_session_handoff_returns_informational() {
    let event_data = test_session_handoff();
    let event = SessionEventData::SessionHandoff(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Informational));
}

// ============================================================================
// DEBUG TIER TESTS: 14 tests
// ============================================================================

/// Session resume is Debug-level state restoration.
#[test]
fn test_classify_session_resume_returns_debug() {
    let event_data = test_session_resume();
    let event = SessionEventData::SessionResume(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Idle state is Debug-level status.
#[test]
fn test_classify_session_idle_returns_debug() {
    let event_data = test_session_idle();
    let event = SessionEventData::SessionIdle(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Session info snapshots are Debug telemetry.
#[test]
fn test_classify_session_info_returns_debug() {
    let event_data = test_session_info();
    let event = SessionEventData::SessionInfo(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Model switching is Debug configuration change.
#[test]
fn test_classify_session_model_change_returns_debug() {
    let event_data = test_session_model_change();
    let event = SessionEventData::SessionModelChange(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Message truncation is Debug memory optimization.
#[test]
fn test_classify_session_truncation_returns_debug() {
    let event_data = test_session_truncation();
    let event = SessionEventData::SessionTruncation(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Pending message changes are Debug state mutations.
#[test]
fn test_classify_pending_messages_modified_returns_debug() {
    let event_data = test_pending_messages_modified();
    let event = SessionEventData::PendingMessagesModified(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Assistant reasoning is Debug-level thinking internals.
#[test]
fn test_classify_assistant_reasoning_returns_debug() {
    let event_data = test_assistant_reasoning();
    let event = SessionEventData::AssistantReasoning(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Reasoning deltas are Debug incremental thinking.
#[test]
fn test_classify_assistant_reasoning_delta_returns_debug() {
    let event_data = test_assistant_reasoning_delta();
    let event = SessionEventData::AssistantReasoningDelta(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Token usage is Debug telemetry.
#[test]
fn test_classify_assistant_usage_returns_debug() {
    let event_data = test_assistant_usage();
    let event = SessionEventData::AssistantUsage(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Partial tool results are Debug intermediate state.
#[test]
fn test_classify_tool_execution_partial_result_returns_debug() {
    let event_data = test_tool_execution_partial_result();
    let event = SessionEventData::ToolExecutionPartialResult(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// System messages are Debug internal communication.
#[test]
fn test_classify_system_message_returns_debug() {
    let event_data = test_system_message();
    let event = SessionEventData::SystemMessage(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Compaction start is Debug memory operation.
#[test]
fn test_classify_session_compaction_start_returns_debug() {
    let event_data = test_session_compaction_start();
    let event = SessionEventData::SessionCompactionStart(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Compaction completion is Debug regardless of success field.
#[test]
fn test_classify_session_compaction_complete_returns_debug() {
    let event_data = test_session_compaction_complete();
    let event = SessionEventData::SessionCompactionComplete(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

/// Snapshot rewinding is Debug recovery operation.
#[test]
fn test_classify_session_snapshot_rewind_returns_debug() {
    let event_data = test_session_snapshot_rewind();
    let event = SessionEventData::SessionSnapshotRewind(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, Some(BackgroundEventPriority::Debug));
}

// ============================================================================
// UNMAPPABLE/NONE TESTS: 2 tests
// ============================================================================

/// SessionUsageInfo is unmappable and returns None.
#[test]
fn test_classify_session_usage_info_returns_none() {
    let event_data = test_session_usage_info();
    let event = SessionEventData::SessionUsageInfo(event_data);
    let priority = classify_event(&event);
    assert_eq!(priority, None);
}

/// Unknown variant is unmappable and returns None.
#[test]
fn test_classify_unknown_returns_none() {
    let event = SessionEventData::Unknown(serde_json::json!({}));
    let priority = classify_event(&event);
    assert_eq!(priority, None);
}

// ============================================================================
// EDGE CASE AND CONDITIONAL LOGIC TESTS: 3 tests
// ============================================================================

/// ToolExecutionComplete returns Informational regardless of success field value.
/// This verifies that the success field does NOT affect classification.
#[test]
fn test_classify_tool_execution_complete_success_true_and_false_both_informational() {
    // Test with success=true
    let mut event_data_true = test_tool_execution_complete();
    event_data_true.success = true;
    let event = SessionEventData::ToolExecutionComplete(event_data_true);
    assert_eq!(
        classify_event(&event),
        Some(BackgroundEventPriority::Informational)
    );

    // Test with success=false
    let mut event_data_false = test_tool_execution_complete();
    event_data_false.success = false;
    let event = SessionEventData::ToolExecutionComplete(event_data_false);
    assert_eq!(
        classify_event(&event),
        Some(BackgroundEventPriority::Informational)
    );
}

/// SessionCompactionComplete returns Debug regardless of success field value.
/// This verifies that the success field does NOT affect classification.
#[test]
fn test_classify_session_compaction_complete_success_true_and_false_both_debug() {
    // Test with success=true
    let mut event_data_true = test_session_compaction_complete();
    event_data_true.success = true;
    let event = SessionEventData::SessionCompactionComplete(event_data_true);
    assert_eq!(classify_event(&event), Some(BackgroundEventPriority::Debug));

    // Test with success=false
    let mut event_data_false = test_session_compaction_complete();
    event_data_false.success = false;
    let event = SessionEventData::SessionCompactionComplete(event_data_false);
    assert_eq!(classify_event(&event), Some(BackgroundEventPriority::Debug));
}

/// All 40 SessionEventData variants are handled by classify() without panicking.
/// This is a comprehensive sanity check that all variants have a classification.
#[test]
fn test_all_40_variants_handled_no_panics() {
    // Critical tier: 6 variants
    let _ = classify_event(&SessionEventData::SessionStart(test_session_start()));
    let _ = classify_event(&SessionEventData::SessionError(test_session_error()));
    let _ = classify_event(&SessionEventData::SessionShutdown(test_session_shutdown()));
    let _ = classify_event(&SessionEventData::Abort(test_abort()));
    let _ = classify_event(&SessionEventData::CustomAgentFailed(
        test_custom_agent_failed(),
    ));
    let _ = classify_event(&SessionEventData::PermissionRequested(
        test_permission_requested(),
    ));

    // Informational tier: 18 variants
    let _ = classify_event(&SessionEventData::UserMessage(test_user_message()));
    let _ = classify_event(&SessionEventData::AssistantTurnStart(
        test_assistant_turn_start(),
    ));
    let _ = classify_event(&SessionEventData::AssistantIntent(test_assistant_intent()));
    let _ = classify_event(&SessionEventData::AssistantMessage(test_assistant_message()));
    let _ = classify_event(&SessionEventData::AssistantMessageDelta(
        test_assistant_message_delta(),
    ));
    let _ = classify_event(&SessionEventData::AssistantTurnEnd(
        test_assistant_turn_end(),
    ));
    let _ = classify_event(&SessionEventData::ToolUserRequested(
        test_tool_user_requested(),
    ));
    let _ = classify_event(&SessionEventData::ToolExecutionStart(
        test_tool_execution_start(),
    ));
    let _ = classify_event(&SessionEventData::ToolExecutionComplete(
        test_tool_execution_complete(),
    ));
    let _ = classify_event(&SessionEventData::ToolExecutionProgress(
        test_tool_execution_progress(),
    ));
    let _ = classify_event(&SessionEventData::CustomAgentStarted(
        test_custom_agent_started(),
    ));
    let _ = classify_event(&SessionEventData::CustomAgentCompleted(
        test_custom_agent_completed(),
    ));
    let _ = classify_event(&SessionEventData::CustomAgentSelected(
        test_custom_agent_selected(),
    ));
    let _ = classify_event(&SessionEventData::HookStart(test_hook_start()));
    let _ = classify_event(&SessionEventData::HookEnd(test_hook_end()));
    let _ = classify_event(&SessionEventData::SkillInvoked(test_skill_invoked()));
    let _ = classify_event(&SessionEventData::ExternalToolRequested(
        test_external_tool_requested(),
    ));
    let _ = classify_event(&SessionEventData::SessionHandoff(test_session_handoff()));

    // Debug tier: 14 variants
    let _ = classify_event(&SessionEventData::SessionResume(test_session_resume()));
    let _ = classify_event(&SessionEventData::SessionIdle(test_session_idle()));
    let _ = classify_event(&SessionEventData::SessionInfo(test_session_info()));
    let _ = classify_event(&SessionEventData::SessionModelChange(
        test_session_model_change(),
    ));
    let _ = classify_event(&SessionEventData::SessionTruncation(
        test_session_truncation(),
    ));
    let _ = classify_event(&SessionEventData::PendingMessagesModified(
        test_pending_messages_modified(),
    ));
    let _ = classify_event(&SessionEventData::AssistantReasoning(
        test_assistant_reasoning(),
    ));
    let _ = classify_event(&SessionEventData::AssistantReasoningDelta(
        test_assistant_reasoning_delta(),
    ));
    let _ = classify_event(&SessionEventData::AssistantUsage(test_assistant_usage()));
    let _ = classify_event(&SessionEventData::ToolExecutionPartialResult(
        test_tool_execution_partial_result(),
    ));
    let _ = classify_event(&SessionEventData::SystemMessage(test_system_message()));
    let _ = classify_event(&SessionEventData::SessionCompactionStart(
        test_session_compaction_start(),
    ));
    let _ = classify_event(&SessionEventData::SessionCompactionComplete(
        test_session_compaction_complete(),
    ));
    let _ = classify_event(&SessionEventData::SessionSnapshotRewind(
        test_session_snapshot_rewind(),
    ));

    // Unmappable/None: 2 variants
    let _ = classify_event(&SessionEventData::SessionUsageInfo(
        test_session_usage_info(),
    ));
    let _ = classify_event(&SessionEventData::Unknown(serde_json::json!({})));
}
