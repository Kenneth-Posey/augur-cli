//! Event Routing Protocols: Specification and validation of 8 event handling protocols.
//!
//! This module defines the 8 protocols that govern how events are classified, routed,
//! and handled across different feeds and contexts. Each protocol provides specific
//! rules for event ordering, suppression, and output formatting.
//!
//! Phase 1.2 deliverable: All 8 protocols with behavioral specifications.

use crate::domain::string_newtypes::EventType;
use crate::domain::{Count, FlushIntervalMs, IsPredicate, SuppressionDecision, TimestampMs};

/// Protocol 1: Rapid Tool Calls
///
/// Handles rapid sequences of tool invocations. Events are queued and ordered
/// to prevent race conditions in tool execution display.
///
/// **Rules**:
/// - Events are accumulated in an ordered queue (FIFO)
/// - Maximum queue depth of 8 tool calls before buffer flush
/// - Batch display when depth threshold reached or 500ms elapsed
#[derive(Clone, Debug)]
pub struct Protocol1RapidToolCalls {
    /// Ordered queue of tool call events (FIFO)
    pub ordered_queue: Vec<EventType>,
    /// Maximum queue depth before forced flush
    pub max_depth: u8,
}

/// Protocol 2: State Machine Violation
///
/// Detects and suppresses events that violate state machine transitions.
/// Prevents display of contradictory state changes or impossible transitions.
///
/// **Rules**:
/// - Tracks current session state and tool call depth
/// - Rejects transitions not in state machine graph
/// - Logs violations to audit trail
#[derive(Clone, Debug)]
pub struct Protocol2StateMachineViolation {
    /// Whether this protocol is aware of session state machine
    pub is_state_machine_aware: IsPredicate,
    /// Violation detection threshold (ms) for rate limiting
    pub violation_threshold_ms: FlushIntervalMs,
}

/// Protocol 3: Recovery Sequencing
///
/// Orders recovery events after errors to ensure consistent display.
/// Sequences error→diagnostic→recovery events in proper order.
///
/// **Rules**:
/// - Errors must be displayed before recovery attempts
/// - Diagnostic events bridge error and recovery
/// - Recovery window of 2 seconds enforced
#[derive(Clone, Debug)]
pub struct Protocol3RecoverySequencing {
    /// Whether event is part of recovery sequence
    pub is_recovery: IsPredicate,
    /// Time window (ms) for error→recovery pairing
    pub error_window_ms: FlushIntervalMs,
}

/// Protocol 4: Snapshot Rewind
///
/// Handles snapshot rewind events that reset session context.
/// Clears buffered output and re-establishes baseline state.
///
/// **Rules**:
/// - Rewind events clear all pending output buffers
/// - Rewind is atomic-no partial state visible
/// - Rewind timestamp used for session reset validation
#[derive(Clone, Debug)]
pub struct Protocol4SnapshotRewind {
    /// Whether rewind should clear all output buffers
    pub clear_buffers: IsPredicate,
    /// Rewind timestamp for session validation
    pub rewind_timestamp_ms: TimestampMs,
}

/// Protocol 5: Nested Agent Suppression
///
/// Suppresses events from nested (background) agents in main feed.
/// Routes nested events to agent-specific feeds instead.
///
/// **Rules**:
/// - Events with parent_tool_call_id are nested
/// - Nested events route to `CustomAgentFeed` indexed by agent identifier
/// - Main feed shows only top-level (parent_tool_call_id==null) events
#[derive(Clone, Debug)]
pub struct Protocol5NestedAgentSuppression {
    /// Whether to suppress nested events from main feed
    pub suppress_nested_from_main: SuppressionDecision,
    /// Maximum nesting depth before error
    pub max_nesting_depth: u8,
}

/// Protocol 6: Usage Info Accumulation
///
/// Batches token usage updates to reduce panel churn.
/// Accumulates usage deltas and flushes at turn boundaries.
///
/// **Rules**:
/// - Token deltas accumulated in buffer
/// - Flushed at TurnComplete or 1-second interval
/// - Buffer size limit: 10 accumulated updates
#[derive(Clone, Debug)]
pub struct Protocol6UsageInfoAccumulation {
    /// Accumulated usage deltas (token count changes)
    pub accumulated_deltas: Vec<i32>,
    /// Flush interval (ms) if no TurnComplete event
    pub flush_interval_ms: FlushIntervalMs,
}

/// Protocol 7: Reasoning Delta Reconstruction
///
/// Reconstructs extended thinking (reasoning) streams for display.
/// Handles reasoning deltas and full reasoning blocks.
///
/// **Rules**:
/// - Reasoning deltas accumulated until ReasoningComplete or 2-second timeout
/// - Display mode (Hidden, Display, BackgroundOnly) gates visibility
/// - Reasoning never interrupts main conversation output
#[derive(Clone, Debug)]
pub struct Protocol7ReasoningDeltaReconstruction {
    /// Display mode for reasoning (Hidden, Display, BackgroundOnly)
    pub display_mode: ReasoningDisplayMode,
    /// Reconstruction timeout (ms) before flush
    pub reconstruction_timeout_ms: FlushIntervalMs,
}

/// Display mode for reasoning events (Protocol 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasoningDisplayMode {
    /// Hide reasoning entirely (default)
    Hidden,
    /// Display reasoning in main feed
    Display,
    /// Route reasoning to background feed only
    BackgroundOnly,
}

/// Protocol 8: Custom Agent Merging
///
/// Merges output from multiple custom agents into unified display.
/// Prevents agent context from contaminating main conversation.
///
/// **Rules**:
/// - Each agent has isolated output context
/// - Agent outputs collected in agent-specific feeds
/// - Merging only happens at session boundaries (turn complete)
#[derive(Clone, Debug)]
pub struct Protocol8CustomAgentMerging {
    /// Agent-specific context isolation enabled
    pub context_isolation_enabled: IsPredicate,
    /// Maximum number of concurrent agents
    pub max_concurrent_agents: Count,
}
