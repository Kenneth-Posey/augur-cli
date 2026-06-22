//! Domain tests for event routing specification

use augur_domain::domain::string_newtypes::{EventType, StringNewtype};

/// Test that all 39 unique events have a valid routing decision
#[test]
fn test_all_39_events_have_valid_route() {
    let event_types = vec![
        // Main feed events (13)
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
        // Agent feed events (3)
        "CustomAgentStarted",
        "CustomAgentCompleted",
        "CustomAgentFailed",
        // Config-dependent events (10)
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
        // Always suppressed (13)
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
    ];

    for event_name in event_types {
        let event_type = EventType::new(event_name);
        let _route = augur_domain::domain::events::inventory::base_route(&event_type);
    }
}

/// Test that routing decisions are deterministic (pure function)
#[test]
fn test_routing_deterministic() {
    let event_type = EventType::new("AssistantMessageDelta");

    let route1 = augur_domain::domain::events::inventory::base_route(&event_type);
    let route2 = augur_domain::domain::events::inventory::base_route(&event_type);
    let route3 = augur_domain::domain::events::inventory::base_route(&event_type);

    assert_eq!(route1, route2, "Routing should be deterministic");
    assert_eq!(route2, route3, "Routing should be deterministic");
}
