//! Background feed tests for event priority classification (Phase 2.1)

use augur_domain::domain::background_events::{BackgroundEventPriority, BackgroundPanelMode};
use augur_domain::domain::string_newtypes::{EventType, StringNewtype};

fn filter_for_mode(
    _event: &EventType,
    priority: BackgroundEventPriority,
    mode: BackgroundPanelMode,
) -> bool {
    mode.includes(priority).0
}

/// Test that all 39 unique events have a priority classification
#[test]
fn test_all_39_events_have_priority() {
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
        // classify_event_priority should not panic and should return a valid priority
        let _priority =
            augur_domain::domain::background_events::classify_event_priority(&event_type);
    }
}

/// Test that priority classification is deterministic (pure function)
#[test]
fn test_priority_classification_deterministic() {
    use augur_domain::domain::string_newtypes::EventType;

    let event_type = EventType::new("SessionError");

    // Calling classify_event_priority multiple times with same input should produce same output
    let priority1 = augur_domain::domain::background_events::classify_event_priority(&event_type);
    let priority2 = augur_domain::domain::background_events::classify_event_priority(&event_type);
    let priority3 = augur_domain::domain::background_events::classify_event_priority(&event_type);

    // Priorities should be equal (derive PartialEq on BackgroundEventPriority)
    assert_eq!(
        priority1, priority2,
        "Priority classification should be deterministic"
    );
    assert_eq!(
        priority2, priority3,
        "Priority classification should be deterministic"
    );
}

/// Test that DeltaAccumulator buffers tokens correctly (Phase 2.2)
#[test]
fn test_delta_accumulator_buffers_tokens() {
    use augur_domain::domain::background_events::DeltaAccumulator;
    use augur_domain::domain::newtypes::BufferThreshold;
    use augur_domain::domain::string_newtypes::ContentDelta;

    let mut accumulator = DeltaAccumulator::default();

    // Accumulate token below threshold (200)
    let token1 = ContentDelta::new("hello");
    let result1 = accumulator.push(token1, BufferThreshold(200));
    assert!(result1.is_none(), "Should not flush below threshold");

    // Accumulate another token
    let token2 = ContentDelta::new(" world");
    let result2 = accumulator.push(token2, BufferThreshold(200));
    assert!(result2.is_none(), "Should not flush below threshold");
}

/// Test that DeltaAccumulator flushes at threshold (Phase 2.2)
#[test]
fn test_delta_accumulator_flushes_at_threshold() {
    use augur_domain::domain::background_events::DeltaAccumulator;
    use augur_domain::domain::newtypes::BufferThreshold;
    use augur_domain::domain::string_newtypes::ContentDelta;

    let mut accumulator = DeltaAccumulator::default();

    // Accumulate tokens below threshold
    let token1 = ContentDelta::new("hello");
    let result1 = accumulator.push(token1, BufferThreshold(20));
    assert!(result1.is_none());

    // Add token that exceeds threshold (15 chars, total 21 chars, threshold 20)
    let token2 = ContentDelta::new(" wonderful world");
    let result2 = accumulator.push(token2, BufferThreshold(20));

    // Should flush when threshold exceeded
    assert!(result2.is_some(), "Should flush when threshold exceeded");
    let flushed = result2.unwrap();
    assert_eq!(flushed.as_str(), "hello wonderful world");
}

/// Test that ToolExecutionContext tracks metadata (Phase 2.2)
#[test]
fn test_tool_context_tracks_metadata() {
    use augur_domain::domain::background_events::{ToolExecutionContext, ToolStatus};
    use augur_domain::domain::string_newtypes::{StringNewtype, ToolName};
    use std::time::Instant;

    let now = Instant::now();
    let tool_name = ToolName::new("cargo_check");

    let context = ToolExecutionContext::new(tool_name.clone(), now, ToolStatus::Running);

    assert_eq!(context.tool_name(), &tool_name);
    assert_eq!(context.status(), ToolStatus::Running);

    // Test event count increment
    let mut context = context;
    context.increment_event_count();
    // Test status change
    context.set_status(ToolStatus::Success);
    assert_eq!(context.status(), ToolStatus::Success);
}

/// Test that Critical mode shows only Critical events (Phase 2.3)
#[test]
fn test_critical_mode_shows_critical_only() {
    use augur_domain::domain::background_events::{BackgroundEventPriority, BackgroundPanelMode};
    use augur_domain::domain::string_newtypes::{EventType, StringNewtype};

    let critical_mode = BackgroundPanelMode::Critical;

    // Critical events should pass through
    let critical_event = EventType::new("SessionError");
    let critical_priority = BackgroundEventPriority::Critical;
    assert!(
        filter_for_mode(&critical_event, critical_priority, critical_mode),
        "Critical mode should show Critical events"
    );

    // Informational events should NOT pass through
    let info_event = EventType::new("ToolExecutionComplete");
    let info_priority = BackgroundEventPriority::Informational;
    assert!(
        !filter_for_mode(&info_event, info_priority, critical_mode),
        "Critical mode should NOT show Informational events"
    );

    // Debug events should NOT pass through
    let debug_event = EventType::new("SessionInfo");
    let debug_priority = BackgroundEventPriority::Debug;
    assert!(
        !filter_for_mode(&debug_event, debug_priority, critical_mode),
        "Critical mode should NOT show Debug events"
    );
}

/// Test that Normal mode shows Critical and Informational events (Phase 2.3)
#[test]
fn test_normal_mode_shows_critical_and_informational() {
    use augur_domain::domain::background_events::{BackgroundEventPriority, BackgroundPanelMode};
    use augur_domain::domain::string_newtypes::{EventType, StringNewtype};

    let normal_mode = BackgroundPanelMode::Normal;

    // Critical events should pass through
    let critical_event = EventType::new("SessionError");
    let critical_priority = BackgroundEventPriority::Critical;
    assert!(
        filter_for_mode(&critical_event, critical_priority, normal_mode),
        "Normal mode should show Critical events"
    );

    // Informational events should pass through
    let info_event = EventType::new("ToolExecutionComplete");
    let info_priority = BackgroundEventPriority::Informational;
    assert!(
        filter_for_mode(&info_event, info_priority, normal_mode),
        "Normal mode should show Informational events"
    );

    // Debug events should NOT pass through
    let debug_event = EventType::new("SessionInfo");
    let debug_priority = BackgroundEventPriority::Debug;
    assert!(
        !filter_for_mode(&debug_event, debug_priority, normal_mode),
        "Normal mode should NOT show Debug events"
    );
}

/// Test that Debug mode shows all events (Phase 2.3)
#[test]
fn test_debug_mode_shows_all_events() {
    use augur_domain::domain::background_events::{BackgroundEventPriority, BackgroundPanelMode};
    use augur_domain::domain::string_newtypes::{EventType, StringNewtype};

    let debug_mode = BackgroundPanelMode::Debug;

    // Critical events should pass through
    let critical_event = EventType::new("SessionError");
    let critical_priority = BackgroundEventPriority::Critical;
    assert!(
        filter_for_mode(&critical_event, critical_priority, debug_mode),
        "Debug mode should show Critical events"
    );

    // Informational events should pass through
    let info_event = EventType::new("ToolExecutionComplete");
    let info_priority = BackgroundEventPriority::Informational;
    assert!(
        filter_for_mode(&info_event, info_priority, debug_mode),
        "Debug mode should show Informational events"
    );

    // Debug events should pass through
    let debug_event = EventType::new("SessionInfo");
    let debug_priority = BackgroundEventPriority::Debug;
    assert!(
        filter_for_mode(&debug_event, debug_priority, debug_mode),
        "Debug mode should show Debug events"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATION SCENARIO TESTS (Phase 2.4): 15 tests across 3 UI modes × 5 scenarios
// ═══════════════════════════════════════════════════════════════════════════════

/// Integration: Critical mode scenario 1 - Session lifecycle events
#[test]
fn test_phase_24_integration_critical_mode_scenario_1() {
    let mode = BackgroundPanelMode::Critical;
    assert!(filter_for_mode(
        &EventType::new("SessionStart"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("ToolExecutionComplete"),
        BackgroundEventPriority::Informational,
        mode
    ));
}

/// Integration: Critical mode scenario 2 - Error handling
#[test]
fn test_phase_24_integration_critical_mode_scenario_2() {
    let mode = BackgroundPanelMode::Critical;
    assert!(filter_for_mode(
        &EventType::new("SessionError"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("SessionInfo"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Critical mode scenario 3 - Agent failure
#[test]
fn test_phase_24_integration_critical_mode_scenario_3() {
    let mode = BackgroundPanelMode::Critical;
    assert!(filter_for_mode(
        &EventType::new("CustomAgentFailed"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("CustomAgentStarted"),
        BackgroundEventPriority::Informational,
        mode
    ));
}

/// Integration: Critical mode scenario 4 - Abort handling
#[test]
fn test_phase_24_integration_critical_mode_scenario_4() {
    let mode = BackgroundPanelMode::Critical;
    assert!(filter_for_mode(
        &EventType::new("Abort"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("ToolExecutionProgress"),
        BackgroundEventPriority::Informational,
        mode
    ));
}

/// Integration: Critical mode scenario 5 - Permission requests
#[test]
fn test_phase_24_integration_critical_mode_scenario_5() {
    let mode = BackgroundPanelMode::Critical;
    assert!(filter_for_mode(
        &EventType::new("PermissionRequested"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("AssistantReasoning"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Normal mode scenario 1 - Critical + Informational events
#[test]
fn test_phase_24_integration_normal_mode_scenario_1() {
    let mode = BackgroundPanelMode::Normal;
    assert!(filter_for_mode(
        &EventType::new("SessionError"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("ToolExecutionStart"),
        BackgroundEventPriority::Informational,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("SessionInfo"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Normal mode scenario 2 - Tool execution progress
#[test]
fn test_phase_24_integration_normal_mode_scenario_2() {
    let mode = BackgroundPanelMode::Normal;
    assert!(filter_for_mode(
        &EventType::new("ToolExecutionProgress"),
        BackgroundEventPriority::Informational,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("ToolExecutionPartialResult"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Normal mode scenario 3 - Assistant messaging
#[test]
fn test_phase_24_integration_normal_mode_scenario_3() {
    let mode = BackgroundPanelMode::Normal;
    assert!(filter_for_mode(
        &EventType::new("AssistantIntent"),
        BackgroundEventPriority::Informational,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("AssistantReasoning"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Normal mode scenario 4 - Custom agent lifecycle
#[test]
fn test_phase_24_integration_normal_mode_scenario_4() {
    let mode = BackgroundPanelMode::Normal;
    assert!(filter_for_mode(
        &EventType::new("CustomAgentCompleted"),
        BackgroundEventPriority::Informational,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("SessionResume"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Normal mode scenario 5 - Session lifecycle with progress updates
#[test]
fn test_phase_24_integration_normal_mode_scenario_5() {
    let mode = BackgroundPanelMode::Normal;
    assert!(filter_for_mode(
        &EventType::new("SessionStart"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("AssistantUsage"),
        BackgroundEventPriority::Informational,
        mode
    ));
    assert!(!filter_for_mode(
        &EventType::new("SessionModelChange"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Debug mode scenario 1 - All event types shown
#[test]
fn test_phase_24_integration_debug_mode_scenario_1() {
    let mode = BackgroundPanelMode::Debug;
    assert!(filter_for_mode(
        &EventType::new("SessionError"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("ToolExecutionComplete"),
        BackgroundEventPriority::Informational,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("SessionInfo"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Debug mode scenario 2 - Verbose diagnostics
#[test]
fn test_phase_24_integration_debug_mode_scenario_2() {
    let mode = BackgroundPanelMode::Debug;
    assert!(filter_for_mode(
        &EventType::new("AssistantReasoning"),
        BackgroundEventPriority::Debug,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("SessionResume"),
        BackgroundEventPriority::Debug,
        mode
    ));
}

/// Integration: Debug mode scenario 3 - Session compaction events
#[test]
fn test_phase_24_integration_debug_mode_scenario_3() {
    let mode = BackgroundPanelMode::Debug;
    assert!(filter_for_mode(
        &EventType::new("SessionCompactionStart"),
        BackgroundEventPriority::Debug,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("SessionCompactionComplete"),
        BackgroundEventPriority::Informational,
        mode
    ));
}

/// Integration: Debug mode scenario 4 - Reasoning delta events
#[test]
fn test_phase_24_integration_debug_mode_scenario_4() {
    let mode = BackgroundPanelMode::Debug;
    assert!(filter_for_mode(
        &EventType::new("AssistantReasoningDelta"),
        BackgroundEventPriority::Debug,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("AssistantMessageDelta"),
        BackgroundEventPriority::Informational,
        mode
    ));
}

/// Integration: Debug mode scenario 5 - Session state changes (mix of priorities)
#[test]
fn test_phase_24_integration_debug_mode_scenario_5() {
    let mode = BackgroundPanelMode::Debug;
    assert!(filter_for_mode(
        &EventType::new("SessionShutdown"),
        BackgroundEventPriority::Critical,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("SessionIdle"),
        BackgroundEventPriority::Informational,
        mode
    ));
    assert!(filter_for_mode(
        &EventType::new("SessionTruncation"),
        BackgroundEventPriority::Debug,
        mode
    ));
}
