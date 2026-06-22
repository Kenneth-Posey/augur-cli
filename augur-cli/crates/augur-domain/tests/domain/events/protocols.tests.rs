use augur_domain::domain::events::protocols::{
    Protocol1RapidToolCalls, Protocol2StateMachineViolation, Protocol3RecoverySequencing,
    Protocol4SnapshotRewind, Protocol5NestedAgentSuppression, Protocol6UsageInfoAccumulation,
    Protocol7ReasoningDeltaReconstruction, Protocol8CustomAgentMerging, ReasoningDisplayMode,
};
use augur_domain::domain::{
    EventType, FlushIntervalMs, IsPredicate, StringNewtype, SuppressionDecision, TimestampMs,
};

const VIOLATION_THRESHOLD_MS: FlushIntervalMs = FlushIntervalMs::of(100);
const ERROR_WINDOW_MS: FlushIntervalMs = FlushIntervalMs::of(2000);
const REWIND_TIMESTAMP_MS: TimestampMs = TimestampMs::of(1_234_567_890);
const FLUSH_INTERVAL_MS: FlushIntervalMs = FlushIntervalMs::of(1000);
const RECONSTRUCTION_TIMEOUT_MS: FlushIntervalMs = FlushIntervalMs::of(2000);

/// Verifies queue order and max depth.
#[test]
fn test_protocol_1_rapid_tool_calls_queue_order() {
    let mut protocol = Protocol1RapidToolCalls {
        ordered_queue: vec![],
        max_depth: 8,
    };
    protocol
        .ordered_queue
        .push(EventType::new("ToolExecutionStart"));
    protocol
        .ordered_queue
        .push(EventType::new("ToolExecutionProgress"));
    protocol
        .ordered_queue
        .push(EventType::new("ToolExecutionComplete"));

    assert_eq!(protocol.ordered_queue.len(), 3);
    assert_eq!(protocol.ordered_queue[0].as_str(), "ToolExecutionStart");
    assert_eq!(protocol.max_depth, 8);
}

/// Verifies state machine violation protocol fields.
#[test]
fn test_protocol_2_state_machine_violation_detection() {
    let protocol = Protocol2StateMachineViolation {
        is_state_machine_aware: IsPredicate::yes(),
        violation_threshold_ms: VIOLATION_THRESHOLD_MS,
    };
    assert!(protocol.is_state_machine_aware.0);
    assert_eq!(protocol.violation_threshold_ms, VIOLATION_THRESHOLD_MS);
}

/// Verifies recovery sequencing protocol fields.
#[test]
fn test_protocol_3_recovery_sequencing() {
    let protocol = Protocol3RecoverySequencing {
        is_recovery: IsPredicate::yes(),
        error_window_ms: ERROR_WINDOW_MS,
    };
    assert!(protocol.is_recovery.0);
    assert_eq!(protocol.error_window_ms, ERROR_WINDOW_MS);
}

/// Verifies snapshot rewind protocol fields.
#[test]
fn test_protocol_4_snapshot_rewind() {
    let protocol = Protocol4SnapshotRewind {
        clear_buffers: IsPredicate::yes(),
        rewind_timestamp_ms: REWIND_TIMESTAMP_MS,
    };
    assert!(protocol.clear_buffers.0);
    assert_eq!(protocol.rewind_timestamp_ms, REWIND_TIMESTAMP_MS);
}

/// Verifies nested agent suppression protocol fields.
#[test]
fn test_protocol_5_nested_agent_suppression() {
    let protocol = Protocol5NestedAgentSuppression {
        suppress_nested_from_main: SuppressionDecision::suppress(),
        max_nesting_depth: 3,
    };
    assert!(protocol.suppress_nested_from_main.0);
    assert_eq!(protocol.max_nesting_depth, 3);
}

/// Verifies usage info accumulation protocol fields.
#[test]
fn test_protocol_6_usage_info_accumulation() {
    let protocol = Protocol6UsageInfoAccumulation {
        accumulated_deltas: vec![10, -5, 15],
        flush_interval_ms: FLUSH_INTERVAL_MS,
    };
    assert_eq!(protocol.accumulated_deltas.len(), 3);
    assert_eq!(protocol.flush_interval_ms, FLUSH_INTERVAL_MS);
}

/// Verifies reasoning delta reconstruction protocol fields.
#[test]
fn test_protocol_7_reasoning_delta_reconstruction() {
    let protocol = Protocol7ReasoningDeltaReconstruction {
        display_mode: ReasoningDisplayMode::Hidden,
        reconstruction_timeout_ms: RECONSTRUCTION_TIMEOUT_MS,
    };
    assert_eq!(protocol.display_mode, ReasoningDisplayMode::Hidden);
    assert_eq!(
        protocol.reconstruction_timeout_ms,
        RECONSTRUCTION_TIMEOUT_MS
    );
}

/// Verifies custom agent merging protocol fields.
#[test]
fn test_protocol_8_custom_agent_merging() {
    let protocol = Protocol8CustomAgentMerging {
        context_isolation_enabled: IsPredicate::yes(),
        max_concurrent_agents: 4usize.into(),
    };
    assert!(protocol.context_isolation_enabled.0);
    assert_eq!(protocol.max_concurrent_agents, 4usize.into());
}

/// Verifies reasoning display mode equality/inequality.
#[test]
fn test_reasoning_display_mode_values() {
    assert_eq!(ReasoningDisplayMode::Hidden, ReasoningDisplayMode::Hidden);
    assert_ne!(ReasoningDisplayMode::Hidden, ReasoningDisplayMode::Display);
    assert_ne!(
        ReasoningDisplayMode::Display,
        ReasoningDisplayMode::BackgroundOnly
    );
}
