//! Background event classification and priority tiers.
//!
//! Part of Feed-Phase-1: Infrastructure & Types.
//! Defines the domain types for event classification and buffering state machines.
//! Classification implementations are provider-owned via `BackgroundEventClassifier`.

use crate::domain::newtypes::{
    AccumulatedContent, BufferThreshold, ErrorMessage, EventCount, ExecutionSuccess, IsDirty,
    IsPredicate, PanelModeLabel, TimestampMs,
};
use crate::domain::string_newtypes::{ContentDelta, DisplayLine, StringNewtype, ToolName};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Provider-owned adapter for mapping raw backend events into domain priority tiers.
///
/// Core domain code depends only on this trait to remain SDK-agnostic for workspace split.
pub trait BackgroundEventClassifier: Send + Sync + 'static {
    fn classify(&self, raw_event: &dyn Any) -> Option<BackgroundEventPriority>;
}

/// Priority tier for background event display.
///
/// Determines which events are shown based on verbosity settings.
/// - `Critical`: Session blockers, user action required (6 variants)
/// - `Informational`: Progress and feedback (18 variants)
/// - `Debug`: Verbose internal details (14 variants)
///
/// # Classification
/// Provider crates implement `BackgroundEventClassifier` to map backend events to these tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundEventPriority {
    /// Tier 1: Blocking events requiring user attention (e.g., SessionStart, SessionError).
    Critical,
    /// Tier 2: Progress and status updates (e.g., ToolExecutionComplete, AssistantMessage).
    Informational,
    /// Tier 3: Verbose internal processing details (e.g., SessionInfo, AssistantReasoning).
    Debug,
}

impl BackgroundEventPriority {
    /// Check if this is a critical priority event.
    ///
    /// Returns an `IsPredicate` wrapper to semantically distinguish priority predicates
    /// from other boolean checks.
    ///
    /// # Example
    /// ```ignore
    /// let priority = BackgroundEventPriority::Critical;
    /// assert!(priority.is_critical().0);
    /// ```
    pub const fn is_critical(&self) -> IsPredicate {
        match self {
            BackgroundEventPriority::Critical => IsPredicate(true),
            _ => IsPredicate(false),
        }
    }

    /// Check if this is an informational priority event.
    ///
    /// Returns an `IsPredicate` wrapper to semantically distinguish priority predicates
    /// from other boolean checks.
    ///
    /// # Example
    /// ```ignore
    /// let priority = BackgroundEventPriority::Informational;
    /// assert!(priority.is_informational().0);
    /// ```
    pub const fn is_informational(&self) -> IsPredicate {
        match self {
            BackgroundEventPriority::Informational => IsPredicate(true),
            _ => IsPredicate(false),
        }
    }

    /// Check if this is a debug priority event.
    ///
    /// Returns an `IsPredicate` wrapper to semantically distinguish priority predicates
    /// from other boolean checks.
    ///
    /// # Example
    /// ```ignore
    /// let priority = BackgroundEventPriority::Debug;
    /// assert!(priority.is_debug().0);
    /// ```
    pub const fn is_debug(&self) -> IsPredicate {
        match self {
            BackgroundEventPriority::Debug => IsPredicate(true),
            _ => IsPredicate(false),
        }
    }
}

/// User-selected verbosity mode for background event panel display.
///
/// Controls which priority tiers are shown based on user preferences.
/// - `Critical`: Show only session blockers (Critical tier)
/// - `Normal`: Show session blockers and progress (Critical + Informational)
/// - `Debug`: Show everything including verbose internal state (all tiers)
///
/// Use `includes()` to check if a given priority should be displayed,
/// and `label()` for UI display strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundPanelMode {
    /// Show only critical events.
    Critical,
    /// Show critical + informational events.
    Normal,
    /// Show all events including debug.
    Debug,
}

impl BackgroundPanelMode {
    /// Check if this mode should display the given priority level.
    ///
    /// Returns an `IsPredicate` indicating whether events with the given priority should be shown
    /// based on this mode's verbosity level.
    ///
    /// # Semantics
    /// - `Critical` mode: shows only Critical priority events (tier 1 blockers)
    /// - `Normal` mode: shows Critical and Informational (tiers 1-2, progress + blockers)
    /// - `Debug` mode: shows all events including Debug (all tiers, full verbosity)
    ///
    /// # Example
    /// ```ignore
    /// let mode = BackgroundPanelMode::Normal;
    /// assert!(mode.includes(BackgroundEventPriority::Critical).0);
    /// assert!(mode.includes(BackgroundEventPriority::Informational).0);
    /// assert!(!mode.includes(BackgroundEventPriority::Debug).0);
    /// ```
    pub fn includes(&self, priority: BackgroundEventPriority) -> IsPredicate {
        let result = match self {
            Self::Critical => priority.is_critical().0,
            Self::Normal => priority.is_critical().0 || priority.is_informational().0,
            Self::Debug => true,
        };
        IsPredicate(result)
    }

    /// Get a display label for this mode.
    ///
    /// Returns a `PanelModeLabel` suitable for UI display, human-readable representation
    /// of this verbosity mode (e.g., "Normal", "Debug", "Critical").
    ///
    /// # Returns
    /// One of `PanelModeLabel` wrapping "Critical", "Normal", or "Debug" depending on the variant.
    ///
    /// # Example
    /// ```ignore
    /// assert_eq!(BackgroundPanelMode::Debug.label().as_str(), "Debug");
    /// assert_eq!(BackgroundPanelMode::Normal.label().as_str(), "Normal");
    /// assert_eq!(BackgroundPanelMode::Critical.label().as_str(), "Critical");
    /// ```
    pub fn label(&self) -> PanelModeLabel {
        let label = match self {
            Self::Critical => "Critical",
            Self::Normal => "Normal",
            Self::Debug => "Debug",
        };
        PanelModeLabel::new(label)
    }
}

/// Filter an event based on priority and UI mode (Phase 2.3).
///
/// Determines whether an event with the given priority should be displayed based on the current
/// panel mode's verbosity settings.
///
/// # Arguments
/// * `_event` - The event type (currently unused, but provided for future extensibility)
/// * `priority` - The classified priority level of the event
/// * `mode` - The current BackgroundPanelMode verbosity setting
///
/// # Returns
/// `true` if the event should be displayed in the given mode, `false` otherwise
///
/// # Mode Semantics
/// - `Critical` mode: Shows only Critical priority events (tier 1 blockers, session lifecycle)
/// - `Normal` mode: Shows Critical and Informational events (progress + blockers, default)
/// - `Debug` mode: Shows all events including Debug (full verbosity for diagnostics)
///
/// # Example
/// ```ignore
/// use domain::background_events::{BackgroundEventPriority, BackgroundPanelMode, filter_for_mode};
/// use domain::string_newtypes::EventType;
///
/// let event = EventType::new("ToolExecutionComplete");
/// let priority = BackgroundEventPriority::Informational;
/// let normal_mode = BackgroundPanelMode::Normal;
///
/// assert!(filter_for_mode(&event, priority, normal_mode));
/// ```
#[allow(dead_code)]
fn filter_for_mode(
    _event: &crate::domain::string_newtypes::EventType,
    priority: BackgroundEventPriority,
    mode: BackgroundPanelMode,
) -> bool {
    mode.includes(priority).0
}

/// Mutable state machine for `AssistantMessageDelta` token buffering.
///
/// Accumulates delta content and flushes when crossing a threshold
/// (default: `DEFAULT_BUFFER_THRESHOLD_CHARS` chars per line, ~200).
/// This enables friendly line-wrapping of streamed assistant responses without breaking mid-token.
///
/// # State
/// - `buffer`: Accumulated string content
/// - `dirty`: Tracks whether buffer has been modified since last flush
///
/// # Example
/// ```ignore
/// let mut acc = DeltaAccumulator::default();
/// let threshold = BufferThreshold::default_threshold();
/// // Accumulate small deltas
/// assert_eq!(acc.push(ContentDelta::new("Hello "), threshold), None);  // Still under threshold
/// assert_eq!(acc.push(ContentDelta::new("world"), threshold), None);   // Still under threshold
/// // Flush manually or when threshold is crossed
/// let content = acc.flush();
/// assert_eq!(content, Some(AccumulatedContent::new("Hello world")));
/// ```
#[derive(Clone, Debug, Default)]
pub struct DeltaAccumulator {
    buffer: String,
    dirty: IsDirty,
}

impl DeltaAccumulator {
    /// Accumulate a delta string, flushing if total exceeds threshold.
    ///
    /// # Arguments
    /// * `delta` - Content delta to append to the buffer (semantic wrapper for streaming chunks).
    /// * `threshold` - Character count limit before auto-flush (see `DEFAULT_BUFFER_THRESHOLD_CHARS`).
    ///   When the accumulated buffer reaches or exceeds this size, the buffer is automatically flushed.
    ///
    /// # Returns
    /// - `Some(flushed_content)` if the buffer exceeded threshold after adding delta.
    ///   The returned `AccumulatedContent` represents all accumulated deltas since the last flush.
    /// - `None` if content remains under threshold and is still buffered.
    ///
    /// # Behavior
    /// This is used for streaming responses to accumulate text in manageable chunks
    /// and flush when a reasonable size is reached, enabling friendly line-wrapping
    /// without breaking mid-token.
    pub fn push(
        &mut self,
        delta: ContentDelta,
        threshold: BufferThreshold,
    ) -> Option<AccumulatedContent> {
        self.buffer.push_str(delta.as_str());
        self.dirty = IsDirty::yes();

        if self.buffer.len() >= threshold.0 {
            self.flush()
        } else {
            None
        }
    }

    /// Manual flush: returns accumulated content and clears buffer.
    ///
    /// # Returns
    /// - `Some(content)` on first call with data. The returned `AccumulatedContent` contains all
    ///   accumulated buffer content since the last flush or since creation.
    ///   This is idempotent in semantics-calling flush multiple times after all content
    ///   has been consumed will return `None` on subsequent calls.
    /// - `None` on subsequent calls after flush (dirty flag is false), or if buffer is empty.
    ///
    /// # Behavior
    /// Once flushed, repeated calls return `None` until new content is accumulated.
    /// This prevents duplicate flushes of stale data.
    ///
    /// # Example
    /// The returned text from flush is typically rendered as a complete line in the UI.
    pub fn flush(&mut self) -> Option<AccumulatedContent> {
        if bool::from(self.dirty) && !self.buffer.is_empty() {
            self.dirty = IsDirty::no();
            Some(AccumulatedContent::new(std::mem::take(&mut self.buffer)))
        } else {
            None
        }
    }

    /// Peek at buffer content without flushing.
    ///
    /// # Returns
    /// - `Some(content)` if buffer contains data as `AccumulatedContent`
    /// - `None` if buffer is empty
    ///
    /// This method does not modify state and does not set the dirty flag.
    pub fn peek(&self) -> Option<AccumulatedContent> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(AccumulatedContent::new(self.buffer.clone()))
        }
    }
}

/// Immutable data carrier for tool invocation tracking.
///
/// Captures the metadata needed to uniquely identify and track a tool execution instance.
/// This struct is typically paired with `ToolExecutionResult` to form the complete
/// execution lifecycle record.
///
/// # Fields
/// * `tool_name`: Identifier of the tool being executed
/// * `tool_args`: JSON representation of invocation arguments
/// * `started_at_ms`: Timestamp in milliseconds since epoch when execution began
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolExecutionMetadata {
    /// Name of the tool being executed.
    pub tool_name: ToolName,
    /// Tool invocation arguments as JSON.
    pub tool_args: serde_json::Value,
    /// Timestamp when execution started (milliseconds since Unix epoch).
    /// Used for calculating elapsed time and performance metrics.
    pub started_at_ms: TimestampMs,
}

impl ToolExecutionMetadata {
    /// Create a new `ToolExecutionMetadata` instance.
    ///
    /// # Arguments
    /// * `tool_name` - Identifier of the tool
    /// * `tool_args` - JSON value containing the tool's invocation arguments
    /// * `started_at_ms` - Timestamp marking execution start in milliseconds since Unix epoch
    ///
    /// # Example
    /// ```ignore
    /// let meta = ToolExecutionMetadata::new(
    ///     ToolName::from("deploy"),
    ///     serde_json::json!({"env": "production"}),
    ///     TimestampMs::now()
    /// );
    /// ```
    pub fn new(
        tool_name: ToolName,
        tool_args: serde_json::Value,
        started_at_ms: TimestampMs,
    ) -> Self {
        Self {
            tool_name,
            tool_args,
            started_at_ms,
        }
    }
}

/// Mutable aggregation state for tool execution result.
///
/// Tracks the outcome of a tool execution and accumulates progress messages
/// during execution. Pairs with `ToolExecutionMetadata` to form a complete
/// execution record.
///
/// # Fields
/// * `success`: Execution success status
/// * `error`: Optional error message if execution failed
/// * `progress_messages`: List of status/progress updates accumulated during execution
///
/// # Example
/// ```ignore
/// let mut result = ToolExecutionResult::new(ExecutionSuccess::failure(), Some(ErrorMessage::new("timeout")));
/// let display = result.to_display_line(ToolName::from("my_tool"));
/// assert!(display.contains("✗"));
/// assert!(display.contains("timeout"));
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// Execution status: indicates successful or failed completion.
    /// Error message should typically be present in the `error` field when success is false.
    pub success: ExecutionSuccess,
    /// Error message describing why execution failed.
    /// Populated when `success` is false; typically None when execution succeeds.
    /// Provides diagnostic information for logging and user feedback.
    pub error: Option<ErrorMessage>,
    /// Progress messages accumulated during tool execution.
    /// Contains status updates, intermediate results, or diagnostic information
    /// produced during the tool's execution lifecycle.
    pub progress_messages: Vec<String>,
}

impl ToolExecutionResult {
    /// Create a new `ToolExecutionResult` instance.
    ///
    /// # Arguments
    /// * `success` - Execution success status
    /// * `error` - Optional error message (typically `None` if success is true)
    ///
    /// Initializes with an empty progress messages vector.
    ///
    /// # Example
    /// ```ignore
    /// let result = ToolExecutionResult::new(ExecutionSuccess::success(), None);
    /// assert!(result.success.0);
    /// assert!(result.error.is_none());
    /// ```
    pub fn new(success: ExecutionSuccess, error: Option<ErrorMessage>) -> Self {
        Self {
            success,
            error,
            progress_messages: Vec::new(),
        }
    }

    /// Format the result as a display line for UI/logging.
    ///
    /// # Arguments
    /// * `tool_name` - The name of the tool to include in the display string
    ///
    /// # Returns
    /// A formatted string like `"✓ tool_name completed"` for success
    /// or `"✗ tool_name failed: error_msg"` for failure.
    ///
    /// # Example
    /// ```ignore
    /// let result = ToolExecutionResult::new(ExecutionSuccess::success(), None);
    /// let line = result.to_display_line(ToolName::from("deploy"));
    /// assert_eq!(line, "✓ deploy completed");
    ///
    /// let result_err = ToolExecutionResult::new(ExecutionSuccess::failure(), Some(ErrorMessage::new("connection lost")));
    /// let line = result_err.to_display_line(ToolName::from("deploy"));
    /// assert!(line.contains("✗ deploy failed: connection lost"));
    /// ```
    pub fn to_display_line(&self, tool_name: ToolName) -> DisplayLine {
        let line = if self.success.0 {
            format!("✓ {} completed", tool_name)
        } else {
            match &self.error {
                Some(err) => format!("✗ {} failed: {}", tool_name, err),
                None => format!("✗ {} failed", tool_name),
            }
        };
        DisplayLine::new(line)
    }
}

/// Maximum number of background events to queue before flushing to the feed.
///
/// Wraps a raw `usize` to prevent accidental mixing with other count types.
/// Used by `StreamFeedConfig` to control buffering behavior during event streaming.
/// When the buffer reaches this capacity, all queued events are flushed to the
/// output stream regardless of elapsed time.
///
/// See `crate::domain::newtypes::QueueCapacity` for the actual type definition.
pub use crate::domain::newtypes::QueueCapacity;

/// Milliseconds between automatic flush intervals for the background event stream.
///
/// Wraps a raw `u64` to prevent accidental mixing with other millisecond values.
/// Used by `StreamFeedConfig` to control periodic flushing of buffered events.
/// When this interval elapses, all buffered events are yielded even if the queue
/// hasn't reached capacity.
///
/// See `crate::domain::newtypes::FlushIntervalMs` for the actual type definition.
pub use crate::domain::newtypes::FlushIntervalMs;

/// Status of tool execution for context tracking (Phase 2.2).
///
/// Tracks the execution state of background tools (e.g., cargo check, clippy).
/// Used by `ToolExecutionContext` to maintain metadata during event processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolStatus {
    /// Tool is currently executing
    Running,
    /// Tool completed successfully
    Success,
    /// Tool execution failed
    Failed,
}

/// Context tracking for tool execution within event streams (Phase 2.2).
///
/// Holds metadata about a specific tool execution session, including tool name,
/// start time, accumulated event count, and execution status.
/// Used by event handlers to correlate related tool events.
#[derive(Clone, Debug)]
pub struct ToolExecutionContext {
    tool_name: ToolName,
    start_time: std::time::Instant,
    event_count: EventCount,
    status: ToolStatus,
}

impl ToolExecutionContext {
    /// Create a new ToolExecutionContext with the given metadata.
    ///
    /// # Arguments
    /// * `tool_name` - The name of the executing tool
    /// * `start_time` - When the tool execution started
    /// * `status` - Current execution status
    pub fn new(tool_name: ToolName, start_time: std::time::Instant, status: ToolStatus) -> Self {
        Self {
            tool_name,
            start_time,
            event_count: EventCount::of(0),
            status,
        }
    }

    /// Get a reference to the tool name.
    pub fn tool_name(&self) -> &ToolName {
        &self.tool_name
    }

    /// Get the number of events associated with this tool execution.
    #[allow(dead_code)]
    fn event_count(&self) -> EventCount {
        self.event_count
    }

    /// Increment the event count by 1.
    pub fn increment_event_count(&mut self) {
        self.event_count += EventCount::of(1);
    }

    /// Get the start time of this tool execution.
    pub fn start_time(&self) -> std::time::Instant {
        self.start_time
    }

    /// Get the current execution status.
    pub fn status(&self) -> ToolStatus {
        self.status
    }

    /// Update the execution status.
    pub fn set_status(&mut self, status: ToolStatus) {
        self.status = status;
    }
}

/// Accumulate a token string into the existing DeltaAccumulator buffer, flushing if threshold exceeded (Phase 2.2).
///
/// This is a convenience wrapper around the DeltaAccumulator::push() method for Phase 2.2 compatibility.
/// Appends the provided token to the internal buffer. If the total length exceeds the configured threshold,
/// returns the accumulated content and resets the buffer.
///
/// # Arguments
/// * `accumulator` - The DeltaAccumulator to update
/// * `token` - The token string to append
///
/// # Returns
/// * `None` if buffer is below threshold after adding the token
/// * `Some(flushed_content)` if buffer exceeded threshold; buffer is reset to empty
///
/// # Note
/// This function is primarily for testing compatibility. Production code should use
/// `DeltaAccumulator::push()` with proper ContentDelta and BufferThreshold types.
#[allow(dead_code)]
fn accumulate_delta(accumulator: &mut DeltaAccumulator, token: String) -> Option<String> {
    use crate::domain::newtypes::BufferThreshold;
    use crate::domain::string_newtypes::ContentDelta;

    // Use the standard default threshold (see DEFAULT_BUFFER_THRESHOLD_CHARS in newtypes).
    let threshold = BufferThreshold::default_threshold();
    let delta = ContentDelta::new(&token);

    accumulator
        .push(delta, threshold)
        .map(|acc_content| acc_content.to_string())
}

/// Manually flush all accumulated tokens from the DeltaAccumulator buffer, resetting it to empty (Phase 2.2).
///
/// This is a convenience wrapper around the DeltaAccumulator::flush() method for Phase 2.2 compatibility.
/// Immediately returns any pending buffered content and clears the internal buffer.
/// Used when a flush is needed before threshold is reached (e.g., on session end).
///
/// # Arguments
/// * `accumulator` - The DeltaAccumulator to flush
///
/// # Returns
/// The accumulated content as a String; buffer is reset to empty
///
/// # Note
/// This function is primarily for testing compatibility. Production code should use
/// `DeltaAccumulator::flush()` which returns `Option&lt;AccumulatedContent&gt;`.
#[allow(dead_code)]
fn flush_accumulated_tokens(accumulator: &mut DeltaAccumulator) -> String {
    accumulator
        .flush()
        .map(|acc_content| acc_content.to_string())
        .unwrap_or_default()
}

/// Deterministic priority classification for background feed events by event type string.
///
/// Maps event type strings (like "SessionError") to priority levels.
/// This function is a complement to provider-owned `BackgroundEventClassifier`
/// implementations. This version works directly with event type strings.
///
/// # Arguments
/// * `event_type` - The event type identifier
///
/// # Returns
/// The BackgroundEventPriority level for this event
pub fn classify_event_priority(
    event_type: &crate::domain::string_newtypes::EventType,
) -> BackgroundEventPriority {
    let event_str: &str = event_type;
    match event_str {
        // Critical events - require immediate attention
        "SessionError"
        | "Abort"
        | "CustomAgentFailed"
        | "PermissionRequested"
        | "SessionStart"
        | "SessionShutdown" => BackgroundEventPriority::Critical,

        // Informational events - provide context and progress
        "AssistantMessageDelta"
        | "SessionIdle"
        | "AssistantIntent"
        | "ToolExecutionStart"
        | "ToolExecutionComplete"
        | "ToolExecutionProgress"
        | "SessionUsageInfo"
        | "SessionCompactionStart"
        | "SessionCompactionComplete"
        | "CustomAgentStarted"
        | "CustomAgentCompleted"
        | "UserMessage"
        | "AssistantTurnStart"
        | "AssistantMessage"
        | "AssistantTurnEnd"
        | "ToolUserRequested"
        | "CustomAgentSelected"
        | "HookStart"
        | "HookEnd"
        | "SkillInvoked"
        | "ExternalToolRequested"
        | "SessionHandoff"
        | "AssistantUsage" => BackgroundEventPriority::Informational,

        // Debug events - for developer inspection and diagnostics
        "AssistantReasoning"
        | "AssistantReasoningDelta"
        | "SessionResume"
        | "SessionInfo"
        | "SessionModelChange"
        | "SessionTruncation"
        | "PendingMessagesModified"
        | "ToolExecutionPartialResult"
        | "SessionSnapshotRewind" => BackgroundEventPriority::Debug,

        // Unknown/future events - treat as debug
        _ => BackgroundEventPriority::Debug,
    }
}
