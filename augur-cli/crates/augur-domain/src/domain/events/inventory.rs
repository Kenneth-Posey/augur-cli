//! Event Inventory: Complete mapping of all 41 SessionEventData variants to routing decisions.
//!
//! This module defines the event routing infrastructure for Copilot SDK events. It provides:
//!
//! 1. **EventRoute enum**: Routing destination for each event
//! 2. **Event Classification**: Categorization of events by semantic role
//! 3. **Suppression Rules**: Constants encoding default suppression decisions
//! 4. **Mapping Documentation**: Justification for each routing decision
//!
//! ## Overview: 41 Events Inventory
//!
//! All Copilot SDK `SessionEventData` variants are accounted for and mapped:
//!
//! - **13 Main Feed events** (user-facing output): AssistantMessageDelta, SessionIdle, SessionError,
//!   Abort, AssistantIntent, ToolExecutionStart/Complete/Progress/PartialResult, AssistantUsage,
//!   SessionUsageInfo, SessionCompactionStart/Complete
//!
//! - **3 Agent Feed events** (custom agent context): CustomAgentStarted, CustomAgentCompleted,
//!   CustomAgentFailed
//!
//! - **2 Background Feed events** (background agent task tracking): AssistantMessageDelta (status),
//!   SessionIdle (task complete)
//!
//! - **23 Unmapped/Suppressed events**: Lifecycle (SessionStart, Resume, Info, Shutdown, etc.),
//!   Reasoning (AssistantReasoning, ReasoningDelta), Protocol v3 (ToolRequested, ExternalToolRequested,
//!   PermissionRequested), Hooks (HookStart, HookEnd), Skills (SkillInvoked), and Metadata
//!   (UserMessage, PendingMessagesModified, AssistantTurnStart/End, AssistantMessage, CustomAgentSelected)
//!
//! ## Routing Decisions
//!
//! Events are routed based on:
//! - **EventCategory**: Lifecycle, Tool, Usage, Status, Reasoning, Agent Coordination, Metadata
//! - **Feed Target**: MainConversation (user-facing), AgentFeed (background agent), Suppress (hidden)
//! - **State Dependencies**: Some events are suppressed if parent_tool_call_id is set or state is AgentActive
//! - **Configuration Dependencies**: Reasoning visibility, lifecycle verbosity (future)

use crate::domain::newtypes::SuppressionDecision;
use crate::domain::string_newtypes::{EventType, StringNewtype};

struct CategoryRule {
    category: EventCategory,
    event_types: &'static [&'static str],
}

const LIFECYCLE_EVENT_TYPES: &[&str] = &[
    "SessionStart",
    "SessionResume",
    "SessionInfo",
    "SessionShutdown",
    "SessionSnapshotRewind",
    "SessionModelChange",
    "SessionHandoff",
    "SessionTruncation",
];

const TOOL_OPERATION_EVENT_TYPES: &[&str] = &[
    "ToolExecutionStart",
    "ToolExecutionComplete",
    "ToolExecutionProgress",
    "ToolExecutionPartialResult",
    "ToolUserRequested",
    "ExternalToolRequested",
];

const USAGE_ACCOUNTING_EVENT_TYPES: &[&str] = &[
    "AssistantUsage",
    "SessionUsageInfo",
    "SessionCompactionStart",
    "SessionCompactionComplete",
];

const STATUS_EVENT_TYPES: &[&str] = &["SessionIdle", "SessionError", "Abort"];

const REASONING_EVENT_TYPES: &[&str] = &["AssistantReasoning", "AssistantReasoningDelta"];

const AGENT_COORDINATION_EVENT_TYPES: &[&str] = &[
    "CustomAgentStarted",
    "CustomAgentCompleted",
    "CustomAgentFailed",
    "CustomAgentSelected",
    "HookStart",
    "HookEnd",
    "SkillInvoked",
    "PermissionRequested",
];

const METADATA_EVENT_TYPES: &[&str] = &[
    "UserMessage",
    "PendingMessagesModified",
    "AssistantTurnStart",
    "AssistantTurnEnd",
    "AssistantMessage",
    "AssistantIntent",
    "AssistantMessageDelta",
];

const CATEGORY_RULES: &[CategoryRule] = &[
    CategoryRule {
        category: EventCategory::Lifecycle,
        event_types: LIFECYCLE_EVENT_TYPES,
    },
    CategoryRule {
        category: EventCategory::ToolOperation,
        event_types: TOOL_OPERATION_EVENT_TYPES,
    },
    CategoryRule {
        category: EventCategory::UsageAccounting,
        event_types: USAGE_ACCOUNTING_EVENT_TYPES,
    },
    CategoryRule {
        category: EventCategory::StatusEvent,
        event_types: STATUS_EVENT_TYPES,
    },
    CategoryRule {
        category: EventCategory::Reasoning,
        event_types: REASONING_EVENT_TYPES,
    },
    CategoryRule {
        category: EventCategory::AgentCoordination,
        event_types: AGENT_COORDINATION_EVENT_TYPES,
    },
    CategoryRule {
        category: EventCategory::Metadata,
        event_types: METADATA_EVENT_TYPES,
    },
];

/// Semantic category of an event based on its role in the session lifecycle.
///
/// Categories group events with similar output requirements and routing logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventCategory {
    /// Session lifecycle events (start, resume, shutdown, model change, etc.)
    Lifecycle,
    /// Tool execution events (start, complete, progress, result)
    ToolOperation,
    /// Token usage and context accounting events
    UsageAccounting,
    /// Session status transitions (idle, error, abort)
    StatusEvent,
    /// Internal reasoning/thinking (extended thinking, chain-of-thought)
    Reasoning,
    /// Agent coordination (custom agents, hooks, skills)
    AgentCoordination,
    /// User input and internal metadata (not meant for output)
    Metadata,
}

/// Routing destination for an event, determining where output (if any) should be sent.
///
/// The routing decision is deterministic and based on:
/// - Event type category
/// - Suppression rules (always-suppressed events)
/// - State-dependent suppression (tool events when parent_tool_call_id set or AgentActive)
/// - Feed availability (custom agents, background agents)
///
/// See suppression rule constants below for predefined rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventRoute {
    /// Event should be output to the main conversation feed (user-facing).
    ///
    /// Examples: Token, Error, TurnComplete, ToolCallStarted, Reasoning (if enabled)
    MainFeed,

    /// Event should be output to the background agent feed.
    ///
    /// Examples: StatusLine (for background agents), TaskCompleted, nested tool progress
    BackgroundFeed,

    /// Event should not produce any output.
    ///
    /// Suppressed events are used internally (state tracking, validation) but
    /// do not appear in any feed. Examples: UserMessage, metadata events,
    /// configuration-dependent events (reasoning if disabled, lifecycle if verbosity=silent).
    Suppress,

    /// Event requires special routing based on runtime context.
    ///
    /// Used for events whose routing depends on state-machine transitions or
    /// configuration settings. The actor layer determines final destination.
    /// Examples: events requiring checkpoint validation, agent-specific routing.
    ContextDependent,
}

/// Suppression rules for always-suppressed event types.
///
/// These events are suppressed by default and never appear in any feed.
/// This constant list allows static analysis of which events are intentionally hidden.
pub const ALWAYS_SUPPRESSED: &[&str] = &[
    // Metadata events (internal state only)
    "UserMessage",             // Handled by CLI layer directly
    "PendingMessagesModified", // Internal registry update
    "AssistantTurnStart",      // Folded into TurnComplete
    "AssistantTurnEnd",        // Folded into TurnComplete
    "AssistantMessage",        // Token streaming more useful
    "CustomAgentSelected",     // Use CustomAgentStarted instead
    // Protocol v3 prep (not yet implemented)
    "ToolUserRequested",     // Future: protocol v3 tool approval
    "ExternalToolRequested", // Future: when v3 fully adopted
    "PermissionRequested",   // Future: security audit required
    // Hook infrastructure (future)
    "HookStart", // Future: hook registry TBD
    "HookEnd",   // Future: hook registry TBD
    // Skills framework (future)
    "SkillInvoked", // Future: larger skills framework
];

/// 13 events that are always suppressed and never appear in any feed.
///
/// Phase 1.1 classification: Complete event inventory with 3 arrays.
/// These events are intentionally hidden from all feeds (main, agent, background).
/// They may be used internally for state tracking but do not produce user-visible output.
pub const ALWAYS_SUPPRESSED_EVENTS: &[&str] = &[
    "UserMessage",             // Handled by CLI layer directly; no feed output
    "PendingMessagesModified", // Internal registry update; no output
    "AssistantTurnStart",      // Folded into TurnComplete event
    "AssistantTurnEnd",        // Folded into TurnComplete event
    "AssistantMessage",        // Token streaming via AssistantMessageDelta is preferred
    "CustomAgentSelected",     // Use CustomAgentStarted instead for tracking
    "ToolUserRequested",       // Future: Protocol v3 tool approval flow
    "ExternalToolRequested",   // Future: Protocol v3 external tools adoption
    "PermissionRequested",     // Future: Security audit required before display
    "HookStart",               // Future: Hook registry infrastructure TBD
    "HookEnd",                 // Future: Hook registry infrastructure TBD
    "SkillInvoked",            // Future: Part of larger skills framework
    "Unknown",                 // Unknown/unrecognized events; treat as suppressed for safety
];

/// 18 events that are always enabled and routed to main feed or agent feed.
///
/// Phase 1.1 classification: Complete event inventory with 3 arrays.
/// These events always produce output in either the main conversation feed or agent-specific feed.
/// Output visibility depends on state-dependent and configuration-dependent gates applied downstream.
pub const ALWAYS_ENABLED_EVENTS: &[&str] = &[
    // Main conversation feed (user-facing)
    "AssistantMessageDelta",      // Token streaming; user sees model output
    "SessionIdle",                // Session ready for new input
    "SessionError",               // Session error condition; never suppressed
    "Abort",                      // Turn aborted; never suppressed
    "AssistantIntent",            // Model's stated intent for current turn
    "ToolExecutionStart",         // Tool call initiated; visible until state gates suppress
    "ToolExecutionComplete",      // Tool call completed
    "ToolExecutionProgress",      // Live tool execution status
    "ToolExecutionPartialResult", // Streaming tool output
    "AssistantUsage",             // Token count updates; buffered until TurnComplete
    "SessionUsageInfo",           // Live context usage meter
    "SessionCompactionStart",     // Context compaction initiated
    "SessionCompactionComplete",  // Context compaction result (success or error)
    // Agent feed (background agent context)
    "CustomAgentStarted",   // Custom agent spawned; routed to agent feed
    "CustomAgentCompleted", // Custom agent succeeded; routed to agent feed
    "CustomAgentFailed",    // Custom agent failed; routed to agent feed
                            // Note: AssistantMessageDelta and SessionIdle have dual-purpose routing in state/config layers
];

/// 10 events with configuration-dependent routing (feature gates, preferences).
///
/// Phase 1.1 classification: Complete event inventory with 3 arrays.
/// These events' visibility depends on runtime configuration settings:
/// - Lifecycle verbosity: Silent (default), Selective, or Verbose
/// - Reasoning display mode: Hidden (default), Display, or BackgroundOnly
///
/// The routing layer applies these gates *after* base route and state-dependent checks.
pub const GATE_DEPENDENT_EVENTS: &[&str] = &[
    // Lifecycle events (gate: lifecycle_verbosity setting)
    "SessionStart",          // Session created; hidden by default
    "SessionResume",         // Session resumed from checkpoint
    "SessionInfo",           // Session metadata/context info
    "SessionShutdown",       // Session ending gracefully
    "SessionSnapshotRewind", // Snapshot rewind initiated
    "SessionModelChange",    // Model changed mid-session
    "SessionHandoff",        // Handoff to different agent or context
    "SessionTruncation",     // Context truncation for space management
    // Reasoning events (gate: reasoning_mode setting)
    "AssistantReasoning", // Extended thinking/internal reasoning (full block)
    "AssistantReasoningDelta", // Extended thinking streaming delta
];

/// State-dependent suppression rules for tool and message events.
///
/// These events are suppressed if certain conditions are met:
/// - Tool events with `parent_tool_call_id` set are suppressed from main feed
/// - Assistant message deltas when `state == AgentActive` are suppressed from main feed
/// - Tool events when `state` is TaskPending, AgentActive, or AwaitingCompletion
///
/// This is applied as a gate *after* the base EventRoute is determined.
pub const STATE_DEPENDENT_SUPPRESSION: &str = r#"
Tool execution events (ToolExecutionStart, Complete, Progress, PartialResult):
  - Suppress from MainFeed if: has_parent_tool_call_id OR state in {TaskPending, AgentActive, AwaitingCompletion}
  - Route to AgentFeed if: applicable agent context exists

Assistant message events (AssistantMessageDelta):
  - Suppress from MainFeed if: has_parent_tool_call_id OR state == AgentActive
  - Route to BackgroundFeed if: state == AgentActive (for status line)
"#;

/// Configuration-dependent suppression rules (future feature flags).
///
/// These events are suppressed or routed based on runtime configuration:
/// - Reasoning display mode (Hidden, Display, BackgroundOnly)
/// - Lifecycle event verbosity (Silent, Selective, Verbose)
///
/// This is applied as a gate *after* state-dependent checks.
pub const CONFIGURATION_DEPENDENT_SUPPRESSION: &str = r#"
Extended Thinking (AssistantReasoning, AssistantReasoningDelta):
  - reasoning_mode == Hidden (default): Suppress entirely
  - reasoning_mode == Display: Route to MainFeed as Reasoning output
  - reasoning_mode == BackgroundOnly: Route to BackgroundFeed only

Lifecycle Events (SessionStart, Resume, Shutdown, etc.):
  - lifecycle_verbosity == Silent (default): Suppress entirely
  - lifecycle_verbosity == Selective: Show only critical transitions (Resume on error, SessionError+recovery)
  - lifecycle_verbosity == Verbose: Show all lifecycle events (SessionStart, Shutdown, ModelChange, etc.)
"#;

/// Complete event inventory: all 41 SessionEventData variants with routing decisions.
///
/// Organized by category and routing destination for clarity.
///
/// ### Main Feed (User-Facing Output) - 13 Events
///
/// These events produce immediately-visible output in the main conversation feed.
/// None are suppressed by default (though state-dependent suppression applies).
///
/// | Event Type | Output Type | Category | Notes |
/// |---|---|---|---|
/// | `AssistantMessageDelta` | `Token` | ToolOperation | Streaming text chunks; suppressed if parent_tool_call_id or AgentActive |
/// | `SessionIdle` | `TurnComplete` | StatusEvent | Session ready for new input |
/// | `SessionError` | `Error` | StatusEvent | Never suppressed |
/// | `Abort` | `Error` | StatusEvent | Never suppressed |
/// | `AssistantIntent` | `IntentMessage` | StatusEvent | Model's stated intent for turn |
/// | `ToolExecutionStart` | `ToolCallStarted` | ToolOperation | Top-level tool name visible; suppressed if parent or AgentActive |
/// | `ToolExecutionComplete` | `ToolCallCompleted` | ToolOperation | Tool result summary; suppressed if parent or state |
/// | `ToolExecutionProgress` | `ToolProgress` | ToolOperation | Live tool status; suppressed if parent or AgentActive |
/// | `ToolExecutionPartialResult` | `ToolPartialResult` | ToolOperation | Streaming tool output; suppressed if parent or AgentActive |
/// | `AssistantUsage` | `UsageUpdate` | UsageAccounting | Token counts; buffered until TurnComplete |
/// | `SessionUsageInfo` | `ContextUsage` | UsageAccounting | Live context meter (current/limit) |
/// | `SessionCompactionStart` | `SystemMessage` | UsageAccounting | Context compaction started |
/// | `SessionCompactionComplete` | `CompactionComplete` or `Error` | UsageAccounting | Context compaction result |
///
/// ### Agent Feed (Background Agent Context) - 3 Events
///
/// These events are routed to the custom agent feed for agent-specific tracking.
///
/// | Event Type | Output Type | Feed | Notes |
/// |---|---|---|---|
/// | `CustomAgentStarted` | `TaskStarted` | AgentFeed\[agent_id\] | Background agent spawned |
/// | `CustomAgentCompleted` | `TaskCompleted` | AgentFeed\[agent_id\] | Background agent succeeded |
/// | `CustomAgentFailed` | `TaskFailed` | AgentFeed\[agent_id\] | Background agent failed |
///
/// ### Background Feed (Status-Only) - 2 Events
///
/// These are dual-routed: `AssistantMessageDelta` and `SessionIdle` have special handling
/// in background agent context (status line, not main feed).
///
/// | Event Type | Output Type | Context | Notes |
/// |---|---|---|---|
/// | `AssistantMessageDelta` | `StatusLine` | When AgentActive | Status update in agent panel |
/// | `SessionIdle` | `TaskCompleted` | When AgentActive | Task completion signal |
///
/// ### Lifecycle Events (Config-Dependent) - 13 Events
///
/// These events are suppressed by default but can be enabled via `lifecycle_verbosity` setting.
/// Proposed output types from Part 1 domain types.
///
/// | Event Type | Output Type (Proposed) | Status | Decision Gate |
/// |---|---|---|---|
/// | `SessionStart` | `SessionStarted` | Proposed | Show in main + background? (medium priority) |
/// | `SessionResume` | `SessionResumed` | Proposed | Show in background only? (medium priority) |
/// | `SessionInfo` | `SessionInfo` | Proposed | Show as session context? (medium priority) |
/// | `SessionShutdown` | (Custom type TBD) | Proposed | Show to user? (low priority) |
/// | `SessionSnapshotRewind` | `SnapshotRewind` | Proposed | Informational only? (low priority) |
/// | `SessionModelChange` | (Custom type TBD) | Proposed | Show prominently? (medium priority) |
/// | `SessionHandoff` | (Custom type TBD) | Proposed | Future: custom agents? (low priority) |
/// | `SessionTruncation` | (Custom type TBD) | Proposed | Visible or background? (medium priority) |
/// | `UserMessage` | **SUPPRESS** | Confirmed | Handled by CLI directly |
/// | `PendingMessagesModified` | **SUPPRESS** | Confirmed | Internal state tracking |
/// | `AssistantTurnStart` | **SUPPRESS** | Confirmed | Folded into TurnComplete |
/// | `AssistantTurnEnd` | **SUPPRESS** | Confirmed | Folded into TurnComplete |
/// | `AssistantMessage` | **SUPPRESS** | Confirmed | Token streaming more useful |
///
/// ### Reasoning/Extended Thinking (Config-Dependent) - 2 Events
///
/// These events carry internal reasoning that may be hidden or displayed based on
/// `reasoning_mode` configuration (future feature flag).
///
/// | Event Type | Output Type (Proposed) | Current Status | Decision Gate |
/// |---|---|---|---|
/// | `AssistantReasoning` | `Reasoning` | Unmapped | Display extended thinking? (HIGH risk) |
/// | `AssistantReasoningDelta` | `ReasoningDelta` | Unmapped | Stream reasoning or hide? (HIGH risk) |
///
/// **Note**: Extended thinking is computationally expensive and may not be user-facing
/// in all modes. Recommended: UI setting for "show reasoning" + background agent always
/// captures for analysis.
///
/// ### Protocol v3 Tools/Requests (Future) - 4 Events
///
/// These events are preparation for Protocol v3 adoption. Suppressed for now.
///
/// | Event Type | Output Type (Proposed) | Status | Priority | Notes |
/// |---|---|---|---|---|
/// | `ToolUserRequested` | `ToolRequested` | Unmapped | medium | Prep for v3 but not urgent |
/// | `ExternalToolRequested` | `ExternalToolRequest` | Unmapped | low | When v3 fully adopted |
/// | `PermissionRequested` | `PermissionRequest` | Unmapped | low | Security audit needed |
/// | `CustomAgentSelected` | **SUPPRESS** | Confirmed | low | Use CustomAgentStarted instead |
///
/// ### Hooks (Infrastructure) - 2 Events
///
/// These events are future infrastructure callbacks for hook registry and lifecycle.
/// Suppressed pending hook infrastructure implementation.
///
/// | Event Type | Output Type (Proposed) | Status | Priority | Notes |
/// |---|---|---|---|---|
/// | `HookStart` | `HookStarted` | Unmapped | low | Hook registry TBD |
/// | `HookEnd` | `HookCompleted` | Unmapped | low | Depends on HookStart context |
///
/// ### Skills/Agents Extension - 2 Events
///
/// These events are part of the larger skills framework (future feature).
/// Suppressed pending skills infrastructure.
///
/// | Event Type | Output Type (Proposed) | Status | Priority | Notes |
/// |---|---|---|---|---|
/// | `SkillInvoked` | `SkillInvoked` | Unmapped | low | Part of skills framework |
/// | `Unknown` | (Preserved as-is) | Implemented | N/A | Forward compatibility ✅ |
///
/// ## Summary: Event Accounting
///
/// | Category | Count | Events | Routing |
/// |---|---|---|---|
/// | Always Suppressed | 13 | UserMessage, PendingMessagesModified, AssistantTurnStart/End, AssistantMessage, CustomAgentSelected, ToolUserRequested, ExternalToolRequested, PermissionRequested, SkillInvoked, HookStart, HookEnd | Suppress |
/// | Always Enabled (Main Feed) | 18 | AssistantMessageDelta, SessionIdle, SessionError, Abort, AssistantIntent, ToolExecution{Start,Complete,Progress,PartialResult}, AssistantUsage, SessionUsageInfo, SessionCompaction{Start,Complete}, CustomAgent{Started,Completed,Failed} | MainFeed/AgentFeed |
/// | Config-Dependent | 10 | SessionStart/Resume/Info/Shutdown/Truncation/ModelChange/HandoffSnapshotRewind, AssistantReasoning/ReasoningDelta | ContextDependent |
/// | **TOTAL** | **41** | - | - |
///
/// ## Routing Protocol: State-Machine Integration
///
/// Events are routed through the following stages:
///
/// 1. **Categorize**: Determine EventCategory
/// 2. **Base Route**: Apply EventRoute based on category
/// 3. **Suppress Check**: Apply ALWAYS_SUPPRESSED list
/// 4. **State Gate**: Apply STATE_DEPENDENT_SUPPRESSION rules
/// 5. **Config Gate**: Apply CONFIGURATION_DEPENDENT_SUPPRESSION rules (future)
/// 6. **Feed Select**: Determine FeedId (MainConversation, AgentFeed, Suppress)
///
/// This deterministic flow ensures events are routed consistently across the system.
pub fn categorize_event(event_type: &EventType) -> EventCategory {
    category_from_rules(event_type.as_str()).unwrap_or(EventCategory::Metadata)
}

/// Determine the base routing destination for an event type.
///
/// This function applies only the base route decision, without state-dependent
/// or configuration-dependent gates. Those are applied by the routing layer.
///
/// Returns `Some(route)` for events with a known routing destination, or
/// `None` for unknown event types (treat as Suppress for safety).
pub fn base_route(event_type: &EventType) -> Option<EventRoute> {
    if is_always_suppressed(event_type).0 {
        return Some(EventRoute::Suppress);
    }
    Some(route_for_category(categorize_event(event_type)))
}

fn category_from_rules(event_type: &str) -> Option<EventCategory> {
    CATEGORY_RULES
        .iter()
        .find(|rule| rule.event_types.contains(&event_type))
        .map(|rule| rule.category)
}

fn route_for_category(category: EventCategory) -> EventRoute {
    if matches!(
        category,
        EventCategory::ToolOperation | EventCategory::UsageAccounting | EventCategory::StatusEvent
    ) {
        EventRoute::MainFeed
    } else if matches!(category, EventCategory::AgentCoordination) {
        EventRoute::BackgroundFeed
    } else if matches!(
        category,
        EventCategory::Reasoning | EventCategory::Lifecycle
    ) {
        EventRoute::ContextDependent
    } else {
        EventRoute::Suppress
    }
}

/// Return a suppression decision indicating if the event type should always be suppressed.
pub fn is_always_suppressed(event_type: &EventType) -> SuppressionDecision {
    SuppressionDecision(ALWAYS_SUPPRESSED.contains(&event_type.as_str()))
}

/// Return a suppression decision indicating if the event type is configuration-dependent (requires feature gate).
///
/// These events are not suppressed by ALWAYS_SUPPRESSED but are gated by
/// configuration settings (reasoning_mode, lifecycle_verbosity, etc.).
pub fn is_config_dependent(event_type: &EventType) -> SuppressionDecision {
    SuppressionDecision(matches!(
        event_type.as_str(),
        "SessionStart"
            | "SessionResume"
            | "SessionInfo"
            | "SessionShutdown"
            | "SessionSnapshotRewind"
            | "SessionModelChange"
            | "SessionHandoff"
            | "SessionTruncation"
            | "AssistantReasoning"
            | "AssistantReasoningDelta"
    ))
}

/// Return a suppression decision indicating if the event type is state-dependent (requires runtime state checks).
///
/// These events may be suppressed based on the current state machine state
/// (e.g., AgentActive, TaskPending) or presence of parent_tool_call_id.
pub fn is_state_dependent(event_type: &EventType) -> SuppressionDecision {
    SuppressionDecision(matches!(
        event_type.as_str(),
        "AssistantMessageDelta"
            | "ToolExecutionStart"
            | "ToolExecutionComplete"
            | "ToolExecutionProgress"
            | "ToolExecutionPartialResult"
    ))
}

/// Return a suppression decision indicating if this event type has a parent_tool_call_id field.
///
/// Events with parent_tool_call_id are typically nested tool calls or outputs
/// and are routed to the agent feed instead of the main feed.
pub fn has_parent_tool_call_id(event_type: &EventType) -> SuppressionDecision {
    SuppressionDecision(matches!(
        event_type.as_str(),
        "AssistantMessageDelta"
            | "ToolExecutionStart"
            | "ToolExecutionComplete"
            | "ToolExecutionProgress"
            | "ToolExecutionPartialResult"
    ))
}

/// Decision gate: Should this event be displayed in the main feed?
///
/// This is a simplified gate for basic filtering (not state-aware).
/// For state-aware suppression, see `is_state_dependent` and the actor layer.
pub fn displays_in_main_feed(event_type: &EventType) -> SuppressionDecision {
    let is_suppressed = is_always_suppressed(event_type).0 || is_config_dependent(event_type).0;
    SuppressionDecision(is_suppressed)
}

/// Decision gate: Should this event be displayed in the agent feed?
///
/// Agent feed events are typically custom agent lifecycle or status updates.
pub fn displays_in_agent_feed(event_type: &EventType) -> SuppressionDecision {
    SuppressionDecision(matches!(
        event_type.as_str(),
        "CustomAgentStarted" | "CustomAgentCompleted" | "CustomAgentFailed"
    ))
}
