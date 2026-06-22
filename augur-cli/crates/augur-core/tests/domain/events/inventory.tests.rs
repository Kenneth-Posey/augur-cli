use augur_domain::domain::events::inventory::{
    base_route, categorize_event, displays_in_agent_feed, displays_in_main_feed,
    has_parent_tool_call_id, is_always_suppressed, is_config_dependent, is_state_dependent,
    EventCategory, EventRoute, ALWAYS_ENABLED_EVENTS, ALWAYS_SUPPRESSED, ALWAYS_SUPPRESSED_EVENTS,
    GATE_DEPENDENT_EVENTS,
};
use augur_domain::domain::{EventType, StringNewtype};

/// Verifies that the `ALWAYS_SUPPRESSED` constant contains exactly 12 entries.
#[test]
fn test_always_suppressed_count() {
    assert_eq!(ALWAYS_SUPPRESSED.len(), 12);
}

/// Verifies that the `ALWAYS_SUPPRESSED_EVENTS` slice contains exactly 13 entries.
#[test]
fn test_always_suppressed_events_count() {
    assert_eq!(ALWAYS_SUPPRESSED_EVENTS.len(), 13);
}

/// Verifies that the `ALWAYS_ENABLED_EVENTS` slice contains exactly 16 entries.
#[test]
fn test_always_enabled_events_count() {
    assert_eq!(ALWAYS_ENABLED_EVENTS.len(), 16);
}

/// Verifies that the `GATE_DEPENDENT_EVENTS` slice contains exactly 10 entries.
#[test]
fn test_gate_dependent_events_count() {
    assert_eq!(GATE_DEPENDENT_EVENTS.len(), 10);
}

/// Verifies that inventory totals 39 events.
#[test]
fn test_event_inventory_total() {
    let total =
        ALWAYS_SUPPRESSED_EVENTS.len() + ALWAYS_ENABLED_EVENTS.len() + GATE_DEPENDENT_EVENTS.len();
    assert_eq!(total, 39);
}

/// Verifies always-suppressed classifications.
#[test]
fn test_is_always_suppressed() {
    assert!(is_always_suppressed(&EventType::new("UserMessage")).0);
    assert!(is_always_suppressed(&EventType::new("PendingMessagesModified")).0);
    assert!(!is_always_suppressed(&EventType::new("SessionIdle")).0);
    assert!(!is_always_suppressed(&EventType::new("AssistantMessageDelta")).0);
}

/// Verifies config-dependent classifications.
#[test]
fn test_is_config_dependent() {
    assert!(is_config_dependent(&EventType::new("SessionStart")).0);
    assert!(is_config_dependent(&EventType::new("AssistantReasoning")).0);
    assert!(!is_config_dependent(&EventType::new("SessionIdle")).0);
    assert!(!is_config_dependent(&EventType::new("SessionError")).0);
}

/// Verifies state-dependent classifications.
#[test]
fn test_is_state_dependent() {
    assert!(is_state_dependent(&EventType::new("AssistantMessageDelta")).0);
    assert!(is_state_dependent(&EventType::new("ToolExecutionStart")).0);
    assert!(!is_state_dependent(&EventType::new("SessionIdle")).0);
    assert!(!is_state_dependent(&EventType::new("SessionError")).0);
}

/// Verifies parent tool-call ID classification.
#[test]
fn test_has_parent_tool_call_id() {
    assert!(has_parent_tool_call_id(&EventType::new("AssistantMessageDelta")).0);
    assert!(has_parent_tool_call_id(&EventType::new("ToolExecutionStart")).0);
    assert!(!has_parent_tool_call_id(&EventType::new("SessionIdle")).0);
    assert!(!has_parent_tool_call_id(&EventType::new("CustomAgentStarted")).0);
}

/// Verifies main feed classification for representative event types.
#[test]
fn test_displays_in_main_feed() {
    assert!(!displays_in_main_feed(&EventType::new("SessionIdle")).0);
    assert!(!displays_in_main_feed(&EventType::new("SessionError")).0);
    assert!(!displays_in_main_feed(&EventType::new("AssistantMessageDelta")).0);
    assert!(displays_in_main_feed(&EventType::new("UserMessage")).0);
    assert!(displays_in_main_feed(&EventType::new("SessionStart")).0);
}

/// Verifies agent feed classification for representative event types.
#[test]
fn test_displays_in_agent_feed() {
    assert!(displays_in_agent_feed(&EventType::new("CustomAgentStarted")).0);
    assert!(displays_in_agent_feed(&EventType::new("CustomAgentCompleted")).0);
    assert!(!displays_in_agent_feed(&EventType::new("SessionIdle")).0);
    assert!(!displays_in_agent_feed(&EventType::new("UserMessage")).0);
}

/// Verifies event categorization.
#[test]
fn test_categorize_event() {
    assert_eq!(
        categorize_event(&EventType::new("SessionError")),
        EventCategory::StatusEvent
    );
    assert_eq!(
        categorize_event(&EventType::new("ToolExecutionStart")),
        EventCategory::ToolOperation
    );
    assert_eq!(
        categorize_event(&EventType::new("SessionStart")),
        EventCategory::Lifecycle
    );
    assert_eq!(
        categorize_event(&EventType::new("AssistantReasoning")),
        EventCategory::Reasoning
    );
}

/// Verifies always-suppressed base routes.
#[test]
fn test_base_route_always_suppressed() {
    assert_eq!(
        base_route(&EventType::new("UserMessage")),
        Some(EventRoute::Suppress)
    );
    assert_eq!(
        base_route(&EventType::new("PendingMessagesModified")),
        Some(EventRoute::Suppress)
    );
}

/// Verifies main-feed base routes.
#[test]
fn test_base_route_main_feed() {
    assert_eq!(
        base_route(&EventType::new("SessionIdle")),
        Some(EventRoute::MainFeed)
    );
    assert_eq!(
        base_route(&EventType::new("SessionError")),
        Some(EventRoute::MainFeed)
    );
    assert_eq!(
        base_route(&EventType::new("ToolExecutionStart")),
        Some(EventRoute::MainFeed)
    );
}

/// Verifies background-feed base routes.
#[test]
fn test_base_route_background_feed() {
    assert_eq!(
        base_route(&EventType::new("CustomAgentStarted")),
        Some(EventRoute::BackgroundFeed)
    );
    assert_eq!(
        base_route(&EventType::new("CustomAgentCompleted")),
        Some(EventRoute::BackgroundFeed)
    );
}

/// Verifies context-dependent base routes.
#[test]
fn test_base_route_context_dependent() {
    assert_eq!(
        base_route(&EventType::new("SessionStart")),
        Some(EventRoute::ContextDependent)
    );
    assert_eq!(
        base_route(&EventType::new("AssistantReasoning")),
        Some(EventRoute::ContextDependent)
    );
}
