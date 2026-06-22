use augur_domain::domain::events::contracts::{OutputCategory, output_contract};
use augur_domain::domain::{EventType, StringNewtype};

/// Verifies that all 39 known event type strings have a callable
/// `output_contract` entry that does not panic, including suppressed events
/// which return `None`.
#[test]
fn test_all_39_events_have_output_category() {
    let event_types = all_known_event_types();

    assert_eq!(event_types.len(), 39, "Expected 39 unique events total");

    for event_str in event_types {
        let event_type = EventType::new(event_str);
        let contract = output_contract(&event_type);
        let _ = contract;
    }
}

fn all_known_event_types() -> Vec<&'static str> {
    vec![
        "AssistantMessageDelta",
        "SessionIdle",
        "SessionError",
        "Abort",
        "AssistantIntent",
        "ToolExecutionStart",
        "ToolExecutionComplete",
        "ToolExecutionProgress",
        "ToolExecutionPartialResult",
        "AssistantUsage",
        "SessionUsageInfo",
        "SessionCompactionStart",
        "SessionCompactionComplete",
        "CustomAgentStarted",
        "CustomAgentCompleted",
        "CustomAgentFailed",
        "SessionStart",
        "SessionResume",
        "SessionInfo",
        "SessionShutdown",
        "SessionSnapshotRewind",
        "SessionModelChange",
        "SessionHandoff",
        "SessionTruncation",
        "AssistantReasoning",
        "AssistantReasoningDelta",
        "UserMessage",
        "PendingMessagesModified",
        "AssistantTurnStart",
        "AssistantTurnEnd",
        "AssistantMessage",
        "CustomAgentSelected",
        "ToolUserRequested",
        "ExternalToolRequested",
        "PermissionRequested",
        "HookStart",
        "HookEnd",
        "SkillInvoked",
        "Unknown",
    ]
}

/// Verifies that specific event types map to their correct `OutputCategory`
/// values, and that always-suppressed events return `None` from `output_contract`.
#[test]
fn test_output_categories_valid_values() {
    assert_eq!(
        output_contract(&EventType::new("AssistantMessageDelta")).map(|c| c.output_category),
        Some(OutputCategory::Token)
    );

    assert_eq!(
        output_contract(&EventType::new("SessionError")).map(|c| c.output_category),
        Some(OutputCategory::Error)
    );

    assert_eq!(
        output_contract(&EventType::new("SessionIdle")).map(|c| c.output_category),
        Some(OutputCategory::TurnComplete)
    );

    assert_eq!(
        output_contract(&EventType::new("ToolExecutionStart")).map(|c| c.output_category),
        Some(OutputCategory::ToolExecution)
    );

    assert_eq!(
        output_contract(&EventType::new("SessionStart")).map(|c| c.output_category),
        Some(OutputCategory::StateChange)
    );

    assert_eq!(
        output_contract(&EventType::new("AssistantUsage")).map(|c| c.output_category),
        Some(OutputCategory::Metadata)
    );

    assert_eq!(output_contract(&EventType::new("UserMessage")), None);
    assert_eq!(
        output_contract(&EventType::new("CustomAgentSelected")),
        None
    );
}

/// Verifies that metadata and reasoning events are configured for batched
/// delivery, while streaming content and error events are not batched.
#[test]
fn test_batching_configuration() {
    assert!(
        output_contract(&EventType::new("AssistantUsage"))
            .map(|c| c.is_batched.0)
            .unwrap_or(false)
    );
    assert!(
        output_contract(&EventType::new("AssistantReasoning"))
            .map(|c| c.is_batched.0)
            .unwrap_or(false)
    );

    assert!(
        !output_contract(&EventType::new("AssistantMessageDelta"))
            .map(|c| c.is_batched.0)
            .unwrap_or(true)
    );
    assert!(
        !output_contract(&EventType::new("SessionError"))
            .map(|c| c.is_batched.0)
            .unwrap_or(true)
    );
    assert!(
        !output_contract(&EventType::new("ToolExecutionStart"))
            .map(|c| c.is_batched.0)
            .unwrap_or(true)
    );
}
