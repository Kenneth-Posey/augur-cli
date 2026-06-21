//! Numeric domain newtypes.
//!
//! Defines the `NumericNewtype` trait and the `newtype_uint!` / `newtype_f64!`
//! generator macros. Each generated type carries semantic meaning in the type
//! system so that raw primitives cannot be accidentally misused at call sites.

use crate::domain::string_newtypes::StringNewtype;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::Sum;
#[allow(unused_imports)]
use std::ops::{Add, AddAssign, Deref, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use std::time::{SystemTime, UNIX_EPOCH};

/// Common interface shared by all numeric newtype wrappers.
///
/// Provides construction, inner-value access, a typed zero constant, and
/// bounds that allow generic use across calculation modules. Use this trait
/// as a bound in generic functions that must operate on any wrapped numeric.
pub trait NumericNewtype: Copy + PartialOrd + Default + fmt::Display {
    /// The underlying primitive type.
    type Inner;
    /// Wrap a raw primitive value.
    fn new(val: Self::Inner) -> Self;
    /// Unwrap to the raw primitive. Reserved for true boundaries (serde,
    /// external APIs); prefer operator overloads for all arithmetic.
    fn inner(self) -> Self::Inner;
    /// The additive identity for this type.
    const ZERO: Self;
}

/// Single Unicode scalar value used in interactive text buffers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextCharacter(pub char);

/// Generate an unsigned-integer-backed numeric newtype.
///
/// Produces a tuple struct with private inner field, derives, the
/// `NumericNewtype` trait impl, `Add`/`Sub`/`AddAssign`/`SubAssign`, `Sum`,
/// `Display`, `Deref`, and `From<inner>`.
/// Integer types do not implement `Neg`, `Mul<inner>`, or `Div<inner>`.
macro_rules! newtype_uint {
    ($(#[$attr:meta])* $name:ident, $inner:ty) => {
        $(#[$attr])*
        #[derive(
            Clone, Copy, Debug, Default,
            PartialEq, Eq, PartialOrd, Ord,
            serde::Serialize, serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name($inner);

        impl NumericNewtype for $name {
            type Inner = $inner;
            #[inline] fn new(val: $inner) -> Self { $name(val) }
            #[inline] fn inner(self) -> $inner { self.0 }
            const ZERO: Self = $name(0);
        }

        impl $name {
            /// Constructs a typed constant value.
            ///
            /// Use in `const` and `static` contexts where `new()` is not callable.
            /// Prefer `new()` in non-const code.
            pub const fn of(val: $inner) -> Self { $name(val) }
        }

        impl Add for $name {
            type Output = Self;
            #[inline] fn add(self, rhs: Self) -> Self { $name(self.0 + rhs.0) }
        }
        impl AddAssign for $name {
            #[inline] fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; }
        }
        impl Sub for $name {
            type Output = Self;
            #[inline] fn sub(self, rhs: Self) -> Self { $name(self.0 - rhs.0) }
        }
        impl SubAssign for $name {
            #[inline] fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; }
        }
        impl Sum for $name {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                iter.fold($name::ZERO, |a, b| a + b)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl Deref for $name {
            type Target = $inner;
            #[inline] fn deref(&self) -> &$inner { &self.0 }
        }
        impl From<$inner> for $name {
            #[inline] fn from(val: $inner) -> Self { $name(val) }
        }
    };
}

/// Generate an `f64`-backed numeric newtype.
///
/// Same interface as `newtype_uint!` plus `Neg`, scalar `Mul<f64>`,
/// scalar `Div<f64>`, `MulAssign<f64>`, `DivAssign<f64>`, and same-type
/// `Div<Self> -> f64`. Does not derive `Eq` or `Ord` (f64 is not totally ordered).
macro_rules! newtype_f64 {
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(
            Clone, Copy, Debug, Default,
            PartialEq, PartialOrd,
            serde::Serialize, serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(f64);

        impl NumericNewtype for $name {
            type Inner = f64;
            #[inline] fn new(val: f64) -> Self { $name(val) }
            #[inline] fn inner(self) -> f64 { self.0 }
            const ZERO: Self = $name(0.0);
        }

        impl Add for $name {
            type Output = Self;
            #[inline] fn add(self, rhs: Self) -> Self { $name(self.0 + rhs.0) }
        }
        impl AddAssign for $name {
            #[inline] fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; }
        }
        impl Sub for $name {
            type Output = Self;
            #[inline] fn sub(self, rhs: Self) -> Self { $name(self.0 - rhs.0) }
        }
        impl SubAssign for $name {
            #[inline] fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; }
        }
        impl Neg for $name {
            type Output = Self;
            #[inline] fn neg(self) -> Self { $name(-self.0) }
        }
        impl Mul<f64> for $name {
            type Output = Self;
            #[inline] fn mul(self, rhs: f64) -> Self { $name(self.0 * rhs) }
        }
        impl Mul<$name> for f64 {
            type Output = $name;
            #[inline] fn mul(self, rhs: $name) -> $name { $name(self * rhs.0) }
        }
        impl MulAssign<f64> for $name {
            #[inline] fn mul_assign(&mut self, rhs: f64) { self.0 *= rhs; }
        }
        impl Div<f64> for $name {
            type Output = Self;
            #[inline] fn div(self, rhs: f64) -> Self { $name(self.0 / rhs) }
        }
        impl DivAssign<f64> for $name {
            #[inline] fn div_assign(&mut self, rhs: f64) { self.0 /= rhs; }
        }
        impl Div<$name> for $name {
            type Output = f64;
            #[inline] fn div(self, rhs: $name) -> f64 { self.0 / rhs.0 }
        }
        impl Sum for $name {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                iter.fold($name::ZERO, |a, b| a + b)
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl Deref for $name {
            type Target = f64;
            #[inline] fn deref(&self) -> &f64 { &self.0 }
        }
        impl From<f64> for $name {
            #[inline] fn from(val: f64) -> Self { $name(val) }
        }
        impl From<$name> for f64 {
            #[inline] fn from(val: $name) -> Self { val.0 }
        }
        impl PartialEq<f64> for $name {
            #[inline] fn eq(&self, other: &f64) -> bool { self.0 == *other }
        }
        impl PartialEq<$name> for f64 {
            #[inline] fn eq(&self, other: &$name) -> bool { *self == other.0 }
        }
        impl Sub<f64> for $name {
            type Output = f64;
            #[inline] fn sub(self, rhs: f64) -> f64 { self.0 - rhs }
        }
        impl Sub<$name> for f64 {
            type Output = f64;
            #[inline] fn sub(self, rhs: $name) -> f64 { self - rhs.0 }
        }
    };
}

newtype_uint!(
    /// Discrete count of tokens in a request or response.
    TokenCount, u64
);
newtype_uint!(
    /// Discrete count of bytes.
    ByteCount, u64
);
newtype_uint!(
    /// Millisecond-precision wall-clock timestamp.
    TimestampMs, u64
);
newtype_uint!(
    /// Second-precision wall-clock timestamp.
    TimestampSecs, u64
);
newtype_uint!(
    /// Discrete count of items or events.
    Count, usize
);
newtype_uint!(
    /// Count of rendered logical lines in the primary feed.
    LineCount, usize
);
newtype_uint!(
    /// Scroll offset measured in logical lines from the end of a feed.
    ScrollOffset, usize
);
newtype_uint!(
    /// Zero-based index of a phase within a guided plan.
    PhaseIndex, usize
);
newtype_uint!(
    /// Zero-based index of a hook within a phase's hook list.
    HookIndex, usize
);
newtype_uint!(
    /// Zero-based index of a user-selectable choice within a `query_user` overlay.
    ///
    /// Wraps a raw `usize` so that choice positions are not accidentally
    /// interchanged with other index or count types.
    ChoiceIndex, usize
);

newtype_uint!(
    /// Duration in whole seconds to wait before a retry attempt.
    ///
    /// Wraps a raw `u64` so that retry wait durations are not accidentally
    /// interchanged with other `u64` values. Consumed by `StreamChunk::RateLimitRetry`
    /// and the provider retry logic in `retry.rs`. Use `.inner()` to pass the value
    /// to `tokio::time::sleep(Duration::from_secs(...))`.
    WaitSecs, u64
);

newtype_f64!(
    /// LLM sampling temperature.
    ///
    /// Higher values produce more varied output. Wraps a raw `f64` so that
    /// temperature is never accidentally interchanged with other domain floats.
    Temperature
);
newtype_f64!(
    /// Dollar-denominated cost in USD.
    ///
    /// Used for per-turn usage (`LlmTokenCounts.cost_usd`) and accumulated
    /// session totals (`ProjectTokenTotals.cost_usd`).
    UsdCost
);
newtype_f64!(
    /// Fraction of oldest tool-result messages to strip during request compaction.
    ToolResultStripFraction
);

newtype_uint!(
    /// Maximum number of background events to queue before flushing to the feed.
    ///
    /// Wraps a raw `usize` to prevent accidental mixing with other count types.
    /// Used by `StreamFeedConfig` to control buffering behavior during event streaming.
    /// When the buffer reaches this capacity, all queued events are flushed to the
    /// output stream regardless of elapsed time.
    QueueCapacity, usize
);

newtype_uint!(
    /// Milliseconds between automatic flush intervals for the background event stream.
    ///
    /// Wraps a raw `u64` to prevent accidental mixing with other millisecond values.
    /// Used by `StreamFeedConfig` to control periodic flushing of buffered events.
    /// When this interval elapses, all buffered events are yielded even if the queue
    /// hasn't reached capacity.
    FlushIntervalMs, u64
);

// --- New numeric newtypes for Phase 2 primitive cleanup ---

newtype_uint!(
    /// One-based line number in a source file.
    ///
    /// Used by LSP location and symbol types to distinguish line positions
    /// from character offsets or other u32 values.
    LineNumber, u32
);

newtype_uint!(
    /// Zero-based character offset on a line in a source file.
    ///
    /// Used by LSP location types to distinguish character positions
    /// from line numbers or other u32 values.
    CharacterOffset, u32
);

newtype_uint!(
    /// Count of background events accumulated in a tool execution context.
    ///
    /// Distinguishes event counts from other u32 counts like line numbers
    /// or character offsets.
    EventCount, u32
);

newtype_uint!(
    /// Number of messages in the content clear window for compaction.
    ///
    /// Distinguishes window sizes from other u32 values like line numbers
    /// or character offsets.
    ClearWindow, u32
);

newtype_uint!(
    /// Number of messages in the drop protection window for compaction.
    ///
    /// Distinguishes drop protection window sizes from other u32 values.
    DropProtectionWindow, u32
);

newtype_uint!(
    /// Rate budget reserve amount in messages for compaction.
    ///
    /// Distinguishes rate budget reserves from other u32 values.
    RateBudgetReserve, u32
);

newtype_uint!(
    /// Maximum tokens for checkpoint summary generation.
    ///
    /// Distinguishes max token counts from other u32 values.
    MaxTokensCount, u32
);

newtype_f64!(
    /// Ratio (0.0-1.0) of context budget allocated for message retention.
    ///
    /// Used by `CompactionConfig` to control what fraction of the context
    /// window is reserved for retaining messages during compaction.
    ContextBudgetRatio
);

newtype_f64!(
    /// Dollar-denominated cost per million tokens.
    ///
    /// Used by `ProviderCatalogModel` to express per-model pricing without
    /// exposing bare f64 values that could be confused with total cost or
    /// temperature.
    CostPerMtok
);

// --- Tool execution status ---

/// Tool execution status indicating success or failure.
///
/// Represents the outcome of a tool execution with clear semantics:
/// - `Success`: Tool completed normally and produced a result
/// - `Failed(reason)`: Tool execution failed for the given reason
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionStatus {
    /// Tool executed successfully.
    Success,
    /// Tool execution failed with a reason.
    Failed(ErrorMessage),
}

impl ExecutionStatus {
    /// Check if this execution was successful.
    ///
    /// Returns a semantic `ExecutionSuccess` wrapper to distinguish execution outcome
    /// from other boolean predicates like `is_critical()` or `is_informational()`.
    pub fn is_success(&self) -> ExecutionSuccess {
        ExecutionSuccess(matches!(self, ExecutionStatus::Success))
    }

    /// Get the failure reason if this execution failed, or `None` if successful.
    ///
    /// Returns an `ErrorMessage` wrapper to semantically distinguish error descriptions
    /// from raw strings. The error reason is wrapped only when the execution failed;
    /// successful executions return `None`.
    pub fn failure_reason(&self) -> Option<ErrorMessage> {
        match self {
            ExecutionStatus::Success => None,
            ExecutionStatus::Failed(reason) => Some(reason.clone()),
        }
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Success => write!(f, "Success"),
            ExecutionStatus::Failed(reason) => write!(f, "Failed: {}", reason),
        }
    }
}

impl TimestampMs {
    /// Returns the current wall-clock time as a millisecond-precision timestamp.
    ///
    /// This is the single timestamp acquisition site for the entire codebase.
    /// All `Message` constructors call this to stamp creation time.
    pub fn now() -> Self {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        TimestampMs(ms)
    }
}

/// Semantic decision indicating whether an event should be suppressed from feed display.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressionDecision(pub bool);

impl SuppressionDecision {
    /// Returns true if this event should be suppressed from the feed.
    ///
    /// This method returns a plain `bool` rather than a wrapper type because suppression
    /// is a binary gate decision that integrates tightly with feed filtering logic.
    /// Unlike predicates such as `is_critical()` or control-flow booleans, suppression
    /// decisions have a specific operational meaning: whether to omit an event from display.
    ///
    /// # Pattern: Check the inner bool directly
    /// To check if an event should be suppressed, use `.0` directly or pattern match:
    /// ```ignore
    /// let decision = SuppressionDecision::suppress();
    /// assert!(decision.0);  // Direct access to bool
    /// ```
    /// Creates a decision to suppress the event.
    pub fn suppress() -> Self {
        SuppressionDecision(true)
    }

    /// Creates a decision to allow the event through.
    pub fn allow() -> Self {
        SuppressionDecision(false)
    }
}

impl From<bool> for SuppressionDecision {
    fn from(b: bool) -> Self {
        SuppressionDecision(b)
    }
}

impl std::fmt::Display for SuppressionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if self.0 { "suppressed" } else { "allowed" })
    }
}

/// Semantic wrapper indicating whether a tool execution succeeded.
///
/// Distinguishes execution success status from other boolean values in the domain model.
/// Use this type for return values and function parameters that specifically mean
/// "did the tool execute successfully?" to prevent accidental type confusion with
/// other boolean values like `is_critical()` or `is_predicate()`.
///
/// # Examples
/// ```ignore
/// let success = ExecutionSuccess::success();
/// assert!(success.0);
///
/// let failure = ExecutionSuccess::failure();
/// assert!(!failure.0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExecutionSuccess(pub bool);

impl ExecutionSuccess {
    /// Returns an `ExecutionSuccess` indicating successful execution.
    pub fn success() -> Self {
        ExecutionSuccess(true)
    }

    /// Returns an `ExecutionSuccess` indicating failed execution.
    pub fn failure() -> Self {
        ExecutionSuccess(false)
    }
}

impl From<bool> for ExecutionSuccess {
    fn from(b: bool) -> Self {
        ExecutionSuccess(b)
    }
}

impl From<ExecutionSuccess> for bool {
    fn from(value: ExecutionSuccess) -> Self {
        value.0
    }
}

impl std::ops::Not for ExecutionSuccess {
    type Output = bool;

    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::fmt::Display for ExecutionSuccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if self.0 { "success" } else { "failure" })
    }
}

/// Error message describing why a tool execution or action failed.
///
/// Wraps a string error description as a distinct semantic type to prevent
/// accidental confusion with other string values in tool execution contexts.
///
/// Error message describing a tool execution or action failure.
///
/// Provides semantic distinction for error descriptions in contexts where multiple
/// string types are used. This prevents accidentally passing the wrong string
/// (e.g., tool output) where an error message is expected.
///
/// # Examples
/// ```ignore
/// let error = ErrorMessage::new("connection timeout");
/// assert_eq!(error.as_str(), "connection timeout");
/// ```
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    std::hash::Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct ErrorMessage(String);

impl StringNewtype for ErrorMessage {
    #[inline]
    fn new(val: impl Into<String>) -> Self {
        ErrorMessage(val.into())
    }
    #[inline]
    fn as_str(&self) -> &str {
        &self.0
    }
    #[inline]
    fn into_inner(self) -> String {
        self.0
    }
}

impl std::ops::Deref for ErrorMessage {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ErrorMessage {
    #[inline]
    fn from(s: String) -> Self {
        ErrorMessage(s)
    }
}

impl From<&str> for ErrorMessage {
    #[inline]
    fn from(s: &str) -> Self {
        ErrorMessage(s.to_owned())
    }
}

impl PartialEq<&str> for ErrorMessage {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<ErrorMessage> for &str {
    #[inline]
    fn eq(&self, other: &ErrorMessage) -> bool {
        *self == other.0
    }
}

impl PartialEq<String> for ErrorMessage {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<ErrorMessage> for String {
    #[inline]
    fn eq(&self, other: &ErrorMessage) -> bool {
        self == &other.0
    }
}

/// Default character count at which buffered feed content is automatically flushed.
///
/// 200 characters aligns with typical terminal/UI line-wrap widths and provides
/// a balance between flush frequency and memory usage for streamed responses.
pub const DEFAULT_BUFFER_THRESHOLD_CHARS: usize = 200;

/// Character count threshold for flushing accumulated deltas in feed buffers.
///
/// Represents the byte/character count at which buffered content is automatically flushed.
/// Wraps `usize` to prevent accidental confusion with other count types like `LineCount`
/// or indices.
///
/// # Default
/// The default threshold is [`DEFAULT_BUFFER_THRESHOLD_CHARS`] characters, suitable for
/// most UI line-wrapping scenarios.
///
/// # Examples
/// ```ignore
/// let threshold = BufferThreshold::default_threshold();
/// assert_eq!(threshold.0, DEFAULT_BUFFER_THRESHOLD_CHARS);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BufferThreshold(pub usize);

impl BufferThreshold {
    /// Returns the default buffer threshold of [`DEFAULT_BUFFER_THRESHOLD_CHARS`] characters.
    pub fn default_threshold() -> Self {
        BufferThreshold(DEFAULT_BUFFER_THRESHOLD_CHARS)
    }
}

impl From<usize> for BufferThreshold {
    fn from(u: usize) -> Self {
        BufferThreshold(u)
    }
}

impl Default for BufferThreshold {
    fn default() -> Self {
        Self::default_threshold()
    }
}

impl std::fmt::Display for BufferThreshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Content accumulated from streaming deltas, ready to emit to feed.
///
/// Wraps accumulated delta text as a distinct semantic type to prevent confusion
/// with raw strings or other text values.
///
/// # Examples
/// ```ignore
/// let content = AccumulatedContent::new("Hello, world!");
/// assert_eq!(content.as_str(), "Hello, world!");
/// ```
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    std::hash::Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct AccumulatedContent(String);

impl StringNewtype for AccumulatedContent {
    #[inline]
    fn new(val: impl Into<String>) -> Self {
        AccumulatedContent(val.into())
    }
    #[inline]
    fn as_str(&self) -> &str {
        &self.0
    }
    #[inline]
    fn into_inner(self) -> String {
        self.0
    }
}

impl std::ops::Deref for AccumulatedContent {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccumulatedContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for AccumulatedContent {
    #[inline]
    fn from(s: String) -> Self {
        AccumulatedContent(s)
    }
}

impl From<&str> for AccumulatedContent {
    #[inline]
    fn from(s: &str) -> Self {
        AccumulatedContent(s.to_owned())
    }
}

impl PartialEq<&str> for AccumulatedContent {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<AccumulatedContent> for &str {
    #[inline]
    fn eq(&self, other: &AccumulatedContent) -> bool {
        *self == other.0
    }
}

impl PartialEq<String> for AccumulatedContent {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<AccumulatedContent> for String {
    #[inline]
    fn eq(&self, other: &AccumulatedContent) -> bool {
        self == &other.0
    }
}

/// Human-readable label for a background panel display mode.
///
/// Wraps panel mode display strings (e.g., "Critical", "Normal", "Debug") as a semantic type
/// to distinguish from arbitrary static strings.
///
/// # Examples
/// ```ignore
/// let label = PanelModeLabel::new("Critical");
/// assert_eq!(label.as_str(), "Critical");
/// ```
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    std::hash::Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct PanelModeLabel(String);

impl StringNewtype for PanelModeLabel {
    #[inline]
    fn new(val: impl Into<String>) -> Self {
        PanelModeLabel(val.into())
    }
    #[inline]
    fn as_str(&self) -> &str {
        &self.0
    }
    #[inline]
    fn into_inner(self) -> String {
        self.0
    }
}

impl std::ops::Deref for PanelModeLabel {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PanelModeLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for PanelModeLabel {
    #[inline]
    fn from(s: String) -> Self {
        PanelModeLabel(s)
    }
}

impl From<&str> for PanelModeLabel {
    #[inline]
    fn from(s: &str) -> Self {
        PanelModeLabel(s.to_owned())
    }
}

impl PartialEq<&str> for PanelModeLabel {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<PanelModeLabel> for &str {
    #[inline]
    fn eq(&self, other: &PanelModeLabel) -> bool {
        *self == other.0
    }
}

impl PartialEq<String> for PanelModeLabel {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<PanelModeLabel> for String {
    #[inline]
    fn eq(&self, other: &PanelModeLabel) -> bool {
        self == &other.0
    }
}

/// Semantic boolean predicate result.
///
/// Used for predicates like `is_critical()`, `is_informational()`, `is_debug()`, and `includes()`.
/// Wraps `bool` to semantically distinguish predicate queries from execution success checks
/// and other boolean values.
///
/// # Examples
/// ```ignore
/// let predicate = IsPredicate::yes();
/// assert!(predicate.to_bool());
///
/// let predicate = IsPredicate::no();
/// assert!(!predicate.to_bool());
/// ```
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    std::hash::Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct IsPredicate(pub bool);

impl IsPredicate {
    /// Returns an `IsPredicate` indicating a true result.
    pub fn yes() -> Self {
        IsPredicate(true)
    }

    /// Returns an `IsPredicate` indicating a false result.
    pub fn no() -> Self {
        IsPredicate(false)
    }
}

impl From<bool> for IsPredicate {
    fn from(b: bool) -> Self {
        IsPredicate(b)
    }
}

impl std::ops::Not for IsPredicate {
    type Output = bool;

    fn not(self) -> Self::Output {
        !self.0
    }
}

impl From<IsPredicate> for bool {
    fn from(value: IsPredicate) -> Self {
        value.0
    }
}

impl std::fmt::Display for IsPredicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if self.0 { "true" } else { "false" })
    }
}

// --- Semantic bool wrappers for Phase 2 primitive cleanup ---

/// Distinguishes tool result messages from other message types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsToolResult(pub bool);

impl IsToolResult {
    pub fn yes() -> Self {
        IsToolResult(true)
    }
    pub fn no() -> Self {
        IsToolResult(false)
    }
}

impl From<bool> for IsToolResult {
    fn from(b: bool) -> Self {
        IsToolResult(b)
    }
}

impl From<IsToolResult> for bool {
    fn from(value: IsToolResult) -> Self {
        value.0
    }
}

impl std::ops::Not for IsToolResult {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::fmt::Display for IsToolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            if self.0 {
                "tool_result"
            } else {
                "not_tool_result"
            }
        )
    }
}

/// Dirty flag for accumulators and buffers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsDirty(pub bool);

impl IsDirty {
    pub fn yes() -> Self {
        IsDirty(true)
    }
    pub fn no() -> Self {
        IsDirty(false)
    }
}

impl From<bool> for IsDirty {
    fn from(b: bool) -> Self {
        IsDirty(b)
    }
}

impl From<IsDirty> for bool {
    fn from(value: IsDirty) -> Self {
        value.0
    }
}

impl std::ops::Not for IsDirty {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

/// Active/inactive state for spinners, indicators, and similar UI elements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsActive(pub bool);

impl IsActive {
    pub fn yes() -> Self {
        IsActive(true)
    }
    pub fn no() -> Self {
        IsActive(false)
    }
}

impl From<bool> for IsActive {
    fn from(b: bool) -> Self {
        IsActive(b)
    }
}

impl From<IsActive> for bool {
    fn from(value: IsActive) -> Self {
        value.0
    }
}

impl std::ops::Not for IsActive {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

/// Visibility state for UI elements (chat menus, dynamic controls, scroll markers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsVisible(pub bool);

impl IsVisible {
    pub fn yes() -> Self {
        IsVisible(true)
    }
    pub fn no() -> Self {
        IsVisible(false)
    }
}

impl From<bool> for IsVisible {
    fn from(b: bool) -> Self {
        IsVisible(b)
    }
}

impl From<IsVisible> for bool {
    fn from(value: IsVisible) -> Self {
        value.0
    }
}

impl std::ops::Not for IsVisible {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

/// Running/stopped state for plan modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsRunning(pub bool);

impl IsRunning {
    pub fn yes() -> Self {
        IsRunning(true)
    }
    pub fn no() -> Self {
        IsRunning(false)
    }
}

impl From<bool> for IsRunning {
    fn from(b: bool) -> Self {
        IsRunning(b)
    }
}

impl From<IsRunning> for bool {
    fn from(value: IsRunning) -> Self {
        value.0
    }
}

impl std::ops::Not for IsRunning {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

/// Decodable/undecodable state for checkpoint records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsDecodable(pub bool);

impl IsDecodable {
    pub fn yes() -> Self {
        IsDecodable(true)
    }
    pub fn no() -> Self {
        IsDecodable(false)
    }
}

impl From<bool> for IsDecodable {
    fn from(b: bool) -> Self {
        IsDecodable(b)
    }
}

impl From<IsDecodable> for bool {
    fn from(value: IsDecodable) -> Self {
        value.0
    }
}

/// Auto-support flag for endpoint model catalogs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SupportsAuto(pub bool);

impl SupportsAuto {
    pub fn yes() -> Self {
        SupportsAuto(true)
    }
    pub fn no() -> Self {
        SupportsAuto(false)
    }
}

impl From<bool> for SupportsAuto {
    fn from(b: bool) -> Self {
        SupportsAuto(b)
    }
}

impl From<SupportsAuto> for bool {
    fn from(value: SupportsAuto) -> Self {
        value.0
    }
}

/// Enabled/disabled state for configuration flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsEnabled(pub bool);

impl IsEnabled {
    pub fn yes() -> Self {
        IsEnabled(true)
    }
    pub fn no() -> Self {
        IsEnabled(false)
    }
}

impl From<bool> for IsEnabled {
    fn from(b: bool) -> Self {
        IsEnabled(b)
    }
}

impl From<IsEnabled> for bool {
    fn from(value: IsEnabled) -> Self {
        value.0
    }
}

impl std::ops::Not for IsEnabled {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

/// Review-active state for guided plan UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsReviewActive(pub bool);

impl IsReviewActive {
    pub fn yes() -> Self {
        IsReviewActive(true)
    }
    pub fn no() -> Self {
        IsReviewActive(false)
    }
}

impl From<bool> for IsReviewActive {
    fn from(b: bool) -> Self {
        IsReviewActive(b)
    }
}

impl From<IsReviewActive> for bool {
    fn from(value: IsReviewActive) -> Self {
        value.0
    }
}

/// Awaiting-compact state for guided plan UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsAwaitingCompact(pub bool);

impl IsAwaitingCompact {
    pub fn yes() -> Self {
        IsAwaitingCompact(true)
    }
    pub fn no() -> Self {
        IsAwaitingCompact(false)
    }
}

impl From<bool> for IsAwaitingCompact {
    fn from(b: bool) -> Self {
        IsAwaitingCompact(b)
    }
}

impl From<IsAwaitingCompact> for bool {
    fn from(value: IsAwaitingCompact) -> Self {
        value.0
    }
}

/// Thinking state for ask panels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsThinking(pub bool);

impl IsThinking {
    pub fn yes() -> Self {
        IsThinking(true)
    }
    pub fn no() -> Self {
        IsThinking(false)
    }
}

impl From<bool> for IsThinking {
    fn from(b: bool) -> Self {
        IsThinking(b)
    }
}

impl From<IsThinking> for bool {
    fn from(value: IsThinking) -> Self {
        value.0
    }
}

/// Seeded state for ask panels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsSeeded(pub bool);

impl IsSeeded {
    pub fn yes() -> Self {
        IsSeeded(true)
    }
    pub fn no() -> Self {
        IsSeeded(false)
    }
}

impl From<bool> for IsSeeded {
    fn from(b: bool) -> Self {
        IsSeeded(b)
    }
}

impl From<IsSeeded> for bool {
    fn from(value: IsSeeded) -> Self {
        value.0
    }
}

/// Turn-completion state for agent status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsTurnComplete(pub bool);

impl IsTurnComplete {
    pub fn yes() -> Self {
        IsTurnComplete(true)
    }
    pub fn no() -> Self {
        IsTurnComplete(false)
    }
}

impl From<bool> for IsTurnComplete {
    fn from(b: bool) -> Self {
        IsTurnComplete(b)
    }
}

impl From<IsTurnComplete> for bool {
    fn from(value: IsTurnComplete) -> Self {
        value.0
    }
}

/// Usage-reset flag for status bar usage tracking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShouldResetUsage(pub bool);

impl ShouldResetUsage {
    pub fn yes() -> Self {
        ShouldResetUsage(true)
    }
    pub fn no() -> Self {
        ShouldResetUsage(false)
    }
}

impl From<bool> for ShouldResetUsage {
    fn from(b: bool) -> Self {
        ShouldResetUsage(b)
    }
}

impl From<ShouldResetUsage> for bool {
    fn from(value: ShouldResetUsage) -> Self {
        value.0
    }
}

/// Compaction-summary flag for summary blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IsCompactionSummary(pub bool);

impl IsCompactionSummary {
    pub fn yes() -> Self {
        IsCompactionSummary(true)
    }
    pub fn no() -> Self {
        IsCompactionSummary(false)
    }
}

impl From<bool> for IsCompactionSummary {
    fn from(b: bool) -> Self {
        IsCompactionSummary(b)
    }
}

impl From<IsCompactionSummary> for bool {
    fn from(value: IsCompactionSummary) -> Self {
        value.0
    }
}

/// Should-send-request flag for background policy decisions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShouldSendRequest(pub bool);

impl ShouldSendRequest {
    pub fn yes() -> Self {
        ShouldSendRequest(true)
    }
    pub fn no() -> Self {
        ShouldSendRequest(false)
    }
}

impl From<bool> for ShouldSendRequest {
    fn from(b: bool) -> Self {
        ShouldSendRequest(b)
    }
}

impl From<ShouldSendRequest> for bool {
    fn from(value: ShouldSendRequest) -> Self {
        value.0
    }
}

/// Latest-checkpoint-present flag for recovery matrix rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HasLatestCheckpoint(pub bool);

impl HasLatestCheckpoint {
    pub fn yes() -> Self {
        HasLatestCheckpoint(true)
    }
    pub fn no() -> Self {
        HasLatestCheckpoint(false)
    }
}

impl From<bool> for HasLatestCheckpoint {
    fn from(b: bool) -> Self {
        HasLatestCheckpoint(b)
    }
}

impl From<HasLatestCheckpoint> for bool {
    fn from(value: HasLatestCheckpoint) -> Self {
        value.0
    }
}

impl std::ops::Not for IsDecodable {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for SupportsAuto {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for IsReviewActive {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for IsAwaitingCompact {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for IsThinking {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for IsSeeded {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for IsTurnComplete {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for ShouldResetUsage {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for IsCompactionSummary {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for ShouldSendRequest {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}

impl std::ops::Not for HasLatestCheckpoint {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}
