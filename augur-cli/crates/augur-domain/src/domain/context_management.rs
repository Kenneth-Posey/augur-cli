//! Context-management domain model and deterministic compaction/checkpoint operations.

use crate::domain::newtypes::{
    ClearWindow, ContextBudgetRatio, DropProtectionWindow, HasLatestCheckpoint,
    IsCompactionSummary, IsDecodable, IsPredicate, IsToolResult, MaxTokensCount, RateBudgetReserve,
    ShouldSendRequest,
};
use crate::domain::string_newtypes::OutputText;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;
use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, OnceLock};

const CLEAR_MARKER: &str = "[cleared]";
const TOTAL_RATE_SLOTS: u32 = 1;
const SUMMARY_BODY_MAX_ESTIMATED_TOKENS: TokenQuantity = TokenQuantity(500);

static LEASE_STATE: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();
static LEASE_RECORDS: OnceLock<Mutex<HashMap<String, RateLeaseLifecycle>>> = OnceLock::new();
static LEASE_ISSUE_COUNTER: OnceLock<Mutex<u64>> = OnceLock::new();

/// Bundles five pipeline orchestration parameters into a context struct representing
/// execution state for pipeline stage preparation.
///
/// This type reduces function parameter complexity by grouping semantic pipeline state
/// into a single value object. Previously, `prepare_stage2_pipeline_step` and
/// `prepare_next_pipeline_step_impl` accepted 5-6 parameters; with this bundling,
/// they now accept 3 parameters.
///
/// # Invariants
///
/// - `context_budget_tokens` must be >= 0
/// - `stable_prefix_before` must not contain terminal/unterminated sequences
/// - Both snapshots must be from valid execution phases
/// - `config` must specify a valid compaction strategy
///
/// # Example
///
/// ```ignore
/// let context = CompactionPipelineContext {
///     snapshot: current_snapshot,
///     stage1_snapshot: prior_snapshot,
///     config,
///     context_budget_tokens: budget,
///     stable_prefix_before: prefix,
/// };
/// prepare_stage2_pipeline_step(&mut run, &context)?;
/// ```
#[derive(Clone, Debug)]
pub struct CompactionPipelineContext {
    /// Current session state snapshot
    pub snapshot: SessionSnapshot,
    /// Prior stage snapshot for comparison
    pub stage1_snapshot: SessionSnapshot,
    /// Pipeline compaction configuration
    pub config: CompactionConfig,
    /// Remaining context budget (tokens)
    pub context_budget_tokens: TokenCount,
    /// Stable prefix bytes from prior iteration.
    pub stable_prefix_before: StablePrefix,
}

/// Constructor payload for [`CompactionPipelineContext`].
///
/// Keeps `CompactionPipelineContext::new` within the parameter-limit rule by
/// bundling all required stage-preparation inputs into one semantic argument.
pub struct CompactionPipelineContextInit {
    pub snapshot: SessionSnapshot,
    pub stage1_snapshot: SessionSnapshot,
    pub config: CompactionConfig,
    pub context_budget_tokens: TokenCount,
    pub stable_prefix_before: StablePrefix,
}

impl CompactionPipelineContext {
    /// Creates a new CompactionPipelineContext with the given components.
    ///
    /// # Arguments
    ///
    /// - `snapshot`: Current session state snapshot
    /// - `stage1_snapshot`: Prior stage snapshot
    /// - `config`: Pipeline compaction configuration
    /// - `context_budget_tokens`: Remaining context budget
    /// - `stable_prefix_before`: Stable prefix from prior iteration
    pub fn new(init: CompactionPipelineContextInit) -> Self {
        CompactionPipelineContext {
            snapshot: init.snapshot,
            stage1_snapshot: init.stage1_snapshot,
            config: init.config,
            context_budget_tokens: init.context_budget_tokens,
            stable_prefix_before: init.stable_prefix_before,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainValidationError {
    EmptyIdentity(&'static str),
    InvalidTurnPairId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Non-zero ordinal used when constructing validated [`TurnPairId`] values.
pub struct TurnPairOrdinal(NonZeroU32);

impl TryFrom<u32> for TurnPairOrdinal {
    type Error = DomainValidationError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let non_zero = NonZeroU32::new(value).ok_or(DomainValidationError::InvalidTurnPairId)?;
        Ok(Self(non_zero))
    }
}

impl From<TurnPairOrdinal> for u32 {
    fn from(value: TurnPairOrdinal) -> Self {
        value.0.get()
    }
}

impl PartialEq<u32> for TurnPairOrdinal {
    fn eq(&self, other: &u32) -> bool {
        self.0.get() == *other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Turn-age scalar measured in historical turn distance.
pub struct TurnPairAgeTurns(u32);

impl From<u32> for TurnPairAgeTurns {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<TurnPairAgeTurns> for u32 {
    fn from(value: TurnPairAgeTurns) -> Self {
        value.0
    }
}

impl PartialEq<u32> for TurnPairAgeTurns {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Token scalar used for deterministic budget arithmetic boundaries.
pub struct TokenQuantity(u32);

impl From<u32> for TokenQuantity {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<TokenQuantity> for u32 {
    fn from(value: TokenQuantity) -> Self {
        value.0
    }
}

impl PartialEq<u32> for TokenQuantity {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Monotonic ordinal used for checkpoint sequence/version wrappers.
pub struct CheckpointOrdinal(u64);

impl From<u64> for CheckpointOrdinal {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<CheckpointOrdinal> for u64 {
    fn from(value: CheckpointOrdinal) -> Self {
        value.0
    }
}

impl PartialEq<u64> for CheckpointOrdinal {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl Display for CheckpointOrdinal {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Reserved slot count used by Stage 3 lease arbitration.
pub struct RateSlotReserve(u32);

impl From<u32> for RateSlotReserve {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<RateSlotReserve> for u32 {
    fn from(value: RateSlotReserve) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Stable turn-pair identifier used for deterministic ordering and segment math.
pub struct TurnPairId(u32);

impl TurnPairId {
    /// Construct a validated turn-pair identifier from a non-zero ordinal input.
    pub fn new(
        raw: impl TryInto<TurnPairOrdinal, Error = DomainValidationError>,
    ) -> Result<Self, DomainValidationError> {
        Ok(Self(raw.try_into()?.into()))
    }

    /// Attempt to return the underlying non-zero turn-pair ordinal value.
    ///
    /// Fails with: [`DomainValidationError::InvalidTurnPairId`] if the internal value is invalid.
    pub fn try_get(self) -> Result<TurnPairOrdinal, DomainValidationError> {
        TurnPairOrdinal::try_from(self.0)
    }

    /// Return the underlying non-zero turn-pair ordinal value.
    pub fn get(self) -> TurnPairOrdinal {
        self.try_get().unwrap_or(TurnPairOrdinal(NonZeroU32::MIN))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Age (in turns) used by deterministic compaction heuristics.
pub struct TurnPairAge(u32);

impl TurnPairAge {
    /// Construct turn age from the semantic turn-distance scalar.
    pub fn new(raw: impl Into<TurnPairAgeTurns>) -> Self {
        Self(raw.into().into())
    }

    /// Return the semantic turn-distance scalar for this age value.
    pub fn get(self) -> TurnPairAgeTurns {
        TurnPairAgeTurns::from(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Token count wrapper used for prompt and budget accounting.
pub struct TokenCount(u32);

impl TokenCount {
    /// Construct token count from the semantic token-quantity scalar.
    pub fn new(raw: impl Into<TokenQuantity>) -> Self {
        Self(raw.into().into())
    }

    /// Return the semantic token-quantity scalar for this token count.
    pub fn get(self) -> TokenQuantity {
        TokenQuantity::from(self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Opaque logical identifier for a compaction-managed session.
pub struct SessionId(String);

impl SessionId {
    /// Construct a non-empty session identifier.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainValidationError> {
        let value = raw.into();
        if value.trim().is_empty() {
            return Err(DomainValidationError::EmptyIdentity("session_id"));
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Opaque objective identifier used to preserve task continuity.
pub struct ObjectiveId(String);

impl ObjectiveId {
    /// Construct a non-empty objective identifier.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainValidationError> {
        let value = raw.into();
        if value.trim().is_empty() {
            return Err(DomainValidationError::EmptyIdentity("objective_id"));
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Opaque window identifier for rate-slot lease coordination.
pub struct WindowId(String);

impl WindowId {
    /// Construct a non-empty lease-window identifier.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainValidationError> {
        let value = raw.into();
        if value.trim().is_empty() {
            return Err(DomainValidationError::EmptyIdentity("window_id"));
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Opaque lease token returned by the Stage 3 rate-slot arbiter.
pub struct LeaseToken(String);

impl LeaseToken {
    /// Construct a non-empty lease token.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainValidationError> {
        let value = raw.into();
        if value.trim().is_empty() {
            return Err(DomainValidationError::EmptyIdentity("lease_token"));
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionType {
    Main,
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Normal,
    Rewind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageName {
    Design,
    Plan,
    Implement,
    Review,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageEvent {
    StageBoundary(StageName),
    NonBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscriptState {
    Decodable,
    Corrupt,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Immutable stable prefix bytes that must be preserved across compaction.
pub struct StablePrefix {
    pub bytes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// User/assistant message payload plus tool-result classification bit.
pub struct Message {
    pub body: OutputText,
    pub is_tool_result: IsToolResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Turn-pair metadata flags consumed by Stage 1/2 drop protection logic.
pub struct TurnPairMetadata {
    pub protected_recent_window: IsPredicate,
    pub objective_changing: IsPredicate,
    pub excluded_from_clearing: IsPredicate,
    pub low_semantic_density: IsPredicate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Identity pair for a compaction turn.
pub struct TurnPairIdentity {
    pub id: TurnPairId,
    pub objective_id: ObjectiveId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// User/assistant exchange unit used as the compaction granularity.
pub struct TurnPair {
    pub identity: TurnPairIdentity,
    pub user_message: Message,
    pub assistant_message: Message,
    pub age: TurnPairAge,
    pub metadata: TurnPairMetadata,
}

impl Deref for TurnPair {
    type Target = TurnPairIdentity;

    fn deref(&self) -> &Self::Target {
        &self.identity
    }
}

impl DerefMut for TurnPair {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.identity
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Context-window budget state for a snapshot.
pub struct SessionContextWindow {
    pub model_context_limit: TokenCount,
    pub provider_prompt_tokens: Option<TokenCount>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Full deterministic snapshot used as input/output of compaction operations.
pub struct SessionSnapshot {
    pub session_id: SessionId,
    pub session_type: SessionType,
    pub stable_prefix: StablePrefix,
    pub turn_pairs: Vec<TurnPair>,
    pub context_window: SessionContextWindow,
}

impl Deref for SessionSnapshot {
    type Target = SessionContextWindow;

    fn deref(&self) -> &Self::Target {
        &self.context_window
    }
}

impl DerefMut for SessionSnapshot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context_window
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SessionRecord {
    pub snapshot: SessionSnapshot,
    pub lifecycle: SessionRecordLifecycle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Runtime compaction configuration validated by guardrails.
pub struct CompactionConfig {
    pub context_budget_ratio: ContextBudgetRatio,
    pub content_clear_window: ClearWindow,
    pub drop_protection_window: DropProtectionWindow,
    pub rate_budget_reserve: RateBudgetReserve,
    pub checkpoint_summary_max_tokens: MaxTokensCount,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateLeaseLifecycle {
    Available,
    Reserved,
    Consumed(LeaseConsumeReason),
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Tracked lifecycle state for an issued Stage 3 rate-slot lease.
pub struct RateLease {
    pub token: LeaseToken,
    pub lifecycle: RateLeaseLifecycle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResumePrompt {
    pub id: ResumePromptId,
    pub text: String,
    pub lifecycle: ResumePromptLifecycle,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConfigSnapshot {
    pub version: ConfigVersion,
    pub config: CompactionConfig,
    pub estimate: BudgetEstimate,
    pub lifecycle: ConfigSnapshotLifecycle,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ResumePromptId(String);

impl ResumePromptId {
    /// Construct a non-empty resume prompt identifier.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainValidationError> {
        let value = raw.into();
        if value.trim().is_empty() {
            return Err(DomainValidationError::EmptyIdentity("resume_prompt_id"));
        }
        Ok(Self(value))
    }
}

impl Display for ResumePromptId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Canonical base prompt text prior to RPT-1 context block injection.
pub struct BasePromptText(String);

impl From<String> for BasePromptText {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for BasePromptText {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for BasePromptText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Canonical resume prompt text emitted by `build_resume_prompt_rpt1`.
pub struct ResumePromptText(String);

impl AsRef<str> for ResumePromptText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for ResumePromptText {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for ResumePromptText {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResumePromptLifecycle {
    Draft,
    Canonicalized,
    Emitted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ConfigVersion(u64);

impl ConfigVersion {
    /// Construct a config version from semantic checkpoint ordinal input.
    pub(crate) fn new(raw: impl Into<CheckpointOrdinal>) -> Self {
        Self(raw.into().into())
    }

    /// Return the semantic checkpoint ordinal representation of this version.
    pub(crate) fn get(self) -> CheckpointOrdinal {
        CheckpointOrdinal(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfigSnapshotLifecycle {
    Loaded,
    Validated,
    Active,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SessionRecordLifecycle {
    Active,
    CompactionRunning,
    ReadyToSend,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleError {
    InvalidTransition {
        entity: &'static str,
        from: &'static str,
        to: &'static str,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Prompt-budget estimate tuple used by policy and stage gating.
pub struct BudgetEstimate {
    pub estimated_prompt_tokens: TokenCount,
    pub context_budget_tokens: TokenCount,
}

impl ResumePrompt {
    /// Create a draft resume prompt with initial text.
    pub(crate) fn new_draft(id: ResumePromptId, text: impl Into<BasePromptText>) -> Self {
        let text = text.into();
        Self {
            id,
            text: text.0,
            lifecycle: ResumePromptLifecycle::Draft,
        }
    }

    /// Canonicalize prompt text into LF form and transition to canonicalized lifecycle state.
    pub(crate) fn canonicalize(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, ResumePromptLifecycle::Draft) {
            return Err(invalid_transition(
                "resume_prompt",
                resume_prompt_lifecycle_label(self.lifecycle),
                resume_prompt_lifecycle_label(ResumePromptLifecycle::Canonicalized),
            ));
        }
        self.text = normalize_lf(&self.text);
        self.lifecycle = ResumePromptLifecycle::Canonicalized;
        Ok(self)
    }

    /// Transition a canonicalized prompt to emitted lifecycle state.
    pub(crate) fn emit(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, ResumePromptLifecycle::Canonicalized) {
            return Err(invalid_transition(
                "resume_prompt",
                resume_prompt_lifecycle_label(self.lifecycle),
                resume_prompt_lifecycle_label(ResumePromptLifecycle::Emitted),
            ));
        }
        self.lifecycle = ResumePromptLifecycle::Emitted;
        Ok(self)
    }
}

impl ConfigSnapshot {
    /// Create a loaded config snapshot in its initial lifecycle state.
    pub(crate) fn new_loaded(
        version: ConfigVersion,
        config: CompactionConfig,
        estimate: BudgetEstimate,
    ) -> Self {
        Self {
            version,
            config,
            estimate,
            lifecycle: ConfigSnapshotLifecycle::Loaded,
        }
    }

    /// Validate snapshot guardrails and transition to validated lifecycle state.
    pub(crate) fn validate(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, ConfigSnapshotLifecycle::Loaded) {
            return Err(invalid_transition(
                "config_snapshot",
                config_snapshot_lifecycle_label(self.lifecycle),
                config_snapshot_lifecycle_label(ConfigSnapshotLifecycle::Validated),
            ));
        }
        validate_config_guardrails(self.config, RequestKind::Normal)
            .map_err(|_| invalid_transition("config_snapshot", "loaded", "validated"))?;
        self.lifecycle = ConfigSnapshotLifecycle::Validated;
        Ok(self)
    }

    /// Transition a validated snapshot to active lifecycle state.
    pub(crate) fn activate(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, ConfigSnapshotLifecycle::Validated) {
            return Err(invalid_transition(
                "config_snapshot",
                config_snapshot_lifecycle_label(self.lifecycle),
                config_snapshot_lifecycle_label(ConfigSnapshotLifecycle::Active),
            ));
        }
        self.lifecycle = ConfigSnapshotLifecycle::Active;
        Ok(self)
    }

    /// Transition a validated snapshot to rejected lifecycle state.
    pub(crate) fn reject(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, ConfigSnapshotLifecycle::Validated) {
            return Err(invalid_transition(
                "config_snapshot",
                config_snapshot_lifecycle_label(self.lifecycle),
                config_snapshot_lifecycle_label(ConfigSnapshotLifecycle::Rejected),
            ));
        }
        self.lifecycle = ConfigSnapshotLifecycle::Rejected;
        Ok(self)
    }
}

impl SessionRecord {
    /// Construct an active session record.
    pub(crate) fn new_active(snapshot: SessionSnapshot) -> Self {
        Self {
            snapshot,
            lifecycle: SessionRecordLifecycle::Active,
        }
    }

    /// Transition active session record into compaction-running state.
    pub(crate) fn start_compaction(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, SessionRecordLifecycle::Active) {
            return Err(invalid_transition(
                "session_record",
                session_record_lifecycle_label(self.lifecycle),
                session_record_lifecycle_label(SessionRecordLifecycle::CompactionRunning),
            ));
        }
        self.lifecycle = SessionRecordLifecycle::CompactionRunning;
        Ok(self)
    }

    /// Transition compaction-running record into ready-to-send state.
    pub(crate) fn mark_ready_to_send(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, SessionRecordLifecycle::CompactionRunning) {
            return Err(invalid_transition(
                "session_record",
                session_record_lifecycle_label(self.lifecycle),
                session_record_lifecycle_label(SessionRecordLifecycle::ReadyToSend),
            ));
        }
        self.lifecycle = SessionRecordLifecycle::ReadyToSend;
        Ok(self)
    }

    /// Transition compaction-running record into blocked state.
    pub(crate) fn block_send(mut self) -> Result<Self, LifecycleError> {
        if !matches!(self.lifecycle, SessionRecordLifecycle::CompactionRunning) {
            return Err(invalid_transition(
                "session_record",
                session_record_lifecycle_label(self.lifecycle),
                session_record_lifecycle_label(SessionRecordLifecycle::Blocked),
            ));
        }
        self.lifecycle = SessionRecordLifecycle::Blocked;
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateClass {
    PureToolExchange,
    ClearedEmpty,
    LowSemanticDensity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Stage 2 classified turn candidate with retained age for tie-breaking.
pub struct ClassifiedCandidate {
    pub turn_id: TurnPairId,
    pub age: TurnPairAge,
    pub class: CandidateClass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Stage 1 output snapshot after content-clearing pass.
pub struct Stage1Result {
    pub snapshot: SessionSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Stage 2 output listing dropped turn IDs in deterministic order.
pub struct Stage2Result {
    pub dropped_turn_ids: Vec<TurnPairId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Contiguous turn segment selected for Stage 3 summarization.
pub struct DroppableSegment {
    pub start_turn: TurnPairId,
    pub end_turn: TurnPairId,
    pub turn_ids: Vec<TurnPairId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Generated Stage 3 summary block that replaces a dropped segment.
pub struct SummaryBlock {
    pub header: String,
    pub body: String,
    pub compaction_summary: IsCompactionSummary,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Request payload consumed by the Stage 3 summary generator.
pub struct SummaryRequest {
    pub segment: DroppableSegment,
    pub preservation_set: PreservationSet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Required semantic elements that must be preserved in summary text.
pub struct PreservationSet {
    pub required_elements: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage3LeaseDecision {
    Granted(LeaseToken),
    Denied(LeaseDenyReason),
}

pub type LeaseDecision = Stage3LeaseDecision;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaseDenyReason {
    ReserveExhausted,
    SlotUnavailable,
    TokenGenerationFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseConsumeReason {
    Used,
    Failed,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseConsumeResult {
    Consumed,
    AlreadyConsumed,
    UnknownLease,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeKind {
    ProceedWithoutCompaction,
    ProceedWithoutStage3,
    ProceedWithSummary,
    ContextPressureWarning,
    ContextOverflowError,
    SummaryGenerationError,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseIdentifier {
    ProceedWithoutCompaction,
    ProceedWithoutStage3,
    ProceedWithSummary,
    ContextPressureWarning,
    ContextOverflowError,
    SummaryGenerationError,
}

impl ResponseIdentifier {
    fn as_str(self) -> &'static str {
        const IDENTIFIERS: [&str; 6] = [
            "proceed-without-compaction",
            "proceed-without-stage3",
            "proceed-with-summary",
            "context-pressure-warning",
            "context-overflow-error",
            "summary-generation-error",
        ];
        IDENTIFIERS[self as usize]
    }
}

impl OutcomeKind {
    fn response_identifier(self) -> ResponseIdentifier {
        const IDENTIFIERS: [ResponseIdentifier; 6] = [
            ResponseIdentifier::ProceedWithoutCompaction,
            ResponseIdentifier::ProceedWithoutStage3,
            ResponseIdentifier::ProceedWithSummary,
            ResponseIdentifier::ContextPressureWarning,
            ResponseIdentifier::ContextOverflowError,
            ResponseIdentifier::SummaryGenerationError,
        ];
        IDENTIFIERS[self as usize]
    }
}

impl Display for ResponseIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Deterministic response envelope for caller-facing outcome identifiers.
pub struct ResponseEnvelope {
    pub identifier: ResponseIdentifier,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Background-session policy decision derived from budget pressure.
pub struct BackgroundPolicyDecision {
    pub should_send_request: ShouldSendRequest,
    pub outcome: OutcomeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Monotonic checkpoint sequence number used in selection ordering.
pub struct CheckpointSequence(u64);

impl CheckpointSequence {
    /// Construct a checkpoint sequence from semantic checkpoint ordinal input.
    pub fn new(raw: impl Into<CheckpointOrdinal>) -> Self {
        Self(raw.into().into())
    }

    /// Return the semantic checkpoint ordinal for this sequence value.
    pub fn get(self) -> CheckpointOrdinal {
        CheckpointOrdinal(self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Human-readable narrative sections embedded in persisted checkpoints.
pub struct CheckpointNarrative {
    pub context_summary: String,
    pub artifacts: Vec<String>,
    pub decisions: Vec<String>,
    pub open_questions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Monotonic ordering metadata for deterministic checkpoint selection.
pub struct CheckpointOrderingMetadata {
    pub checkpoint_sequence: CheckpointSequence,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Persisted stage-boundary payload used for restart and resume prompts.
pub struct CheckpointPayload {
    pub objective: String,
    pub stage_completed: StageName,
    pub next_stage: StageName,
    pub narrative: CheckpointNarrative,
    pub ordering: CheckpointOrderingMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Composite ordering key for deterministic latest-checkpoint selection.
pub struct CheckpointOrderingKey {
    pub checkpoint_sequence: CheckpointSequence,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointLifecycle {
    Candidate,
    Validated,
    Persisted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Checkpoint record tracked across candidate/validated/persisted lifecycle.
pub struct CheckpointRecord {
    pub payload: CheckpointPayload,
    pub decodable: IsDecodable,
    pub lifecycle: CheckpointLifecycle,
}

impl CheckpointRecord {
    fn new_candidate(payload: CheckpointPayload) -> Self {
        Self {
            payload,
            decodable: IsDecodable::yes(),
            lifecycle: CheckpointLifecycle::Candidate,
        }
    }

    fn transition_to(mut self, next: CheckpointLifecycle) -> Result<Self, CheckpointError> {
        let allowed = matches!(
            (self.lifecycle.clone(), next.clone()),
            (
                CheckpointLifecycle::Candidate,
                CheckpointLifecycle::Validated
            ) | (
                CheckpointLifecycle::Validated,
                CheckpointLifecycle::Persisted
            )
        );
        if !allowed {
            return Err(CheckpointError::CheckpointWriteError);
        }
        self.lifecycle = next;
        Ok(self)
    }

    fn transition_write_failure(mut self) -> Result<Self, CheckpointError> {
        if !matches!(self.lifecycle, CheckpointLifecycle::Validated) {
            return Err(CheckpointError::CheckpointWriteError);
        }
        self.lifecycle = CheckpointLifecycle::Candidate;
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompactionRunState {
    Initialized,
    Stage1Done,
    Stage2Done,
    Stage3Pending,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompactionRun {
    pub session_id: SessionId,
    pub state: CompactionRunState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompactionCompletionReason {
    Stage1WithinBudget,
    Stage2WithinBudget,
    BackgroundPressure,
    EmptyDroppableSegment,
    LeaseDenied,
    SummaryGenerationFailed,
    SummaryContractFailed,
    FinalBudgetOverflow,
    SummaryCommitted,
}

impl CompactionRun {
    fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            state: CompactionRunState::Initialized,
        }
    }

    fn stage1_done(&mut self) -> Result<(), CompactionRunError> {
        if !matches!(self.state, CompactionRunState::Initialized) {
            return Err(CompactionRunError::InvalidStageTransition);
        }
        self.state = CompactionRunState::Stage1Done;
        Ok(())
    }

    fn stage2_done(&mut self) -> Result<(), CompactionRunError> {
        if !matches!(self.state, CompactionRunState::Stage1Done) {
            return Err(CompactionRunError::InvalidStageTransition);
        }
        self.state = CompactionRunState::Stage2Done;
        Ok(())
    }

    fn stage3_pending(&mut self) -> Result<(), CompactionRunError> {
        if !matches!(self.state, CompactionRunState::Stage2Done) {
            return Err(CompactionRunError::InvalidStageTransition);
        }
        self.state = CompactionRunState::Stage3Pending;
        Ok(())
    }

    fn complete(&mut self, reason: CompactionCompletionReason) -> Result<(), CompactionRunError> {
        let allowed = matches!(
            (self.state, reason),
            (
                CompactionRunState::Stage1Done,
                CompactionCompletionReason::Stage1WithinBudget
            ) | (
                CompactionRunState::Stage2Done,
                CompactionCompletionReason::Stage2WithinBudget
                    | CompactionCompletionReason::BackgroundPressure
                    | CompactionCompletionReason::EmptyDroppableSegment
                    | CompactionCompletionReason::LeaseDenied
            ) | (
                CompactionRunState::Stage3Pending,
                CompactionCompletionReason::SummaryGenerationFailed
                    | CompactionCompletionReason::SummaryContractFailed
                    | CompactionCompletionReason::FinalBudgetOverflow
                    | CompactionCompletionReason::SummaryCommitted
            )
        );
        if !allowed {
            return Err(CompactionRunError::InvalidStageTransition);
        }
        self.state = CompactionRunState::Completed;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompactionRunError {
    InvalidStageTransition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryOutcome {
    ResumeFromCheckpoint(CheckpointRecord),
    ResumeFromTranscript,
    ResumeFromTranscriptRetryNeeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidRatio,
    InvalidIntegerField(String),
    RewindOutOfScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompactionError {
    ContextPressureWarning,
    ContextOverflowError,
    SummaryGenerationError,
    InvalidSummaryContract,
    LeaseDenied,
    EmptyDroppableSegment,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointError {
    CheckpointWriteError,
    CheckpointCorruptionError,
    PayloadSchemaError(String),
    SummaryTooLarge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    CheckpointCorruptionError,
    TranscriptCorruptionError,
    MissingSessionStateError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Recovery attempt inputs used by restart matrix evaluation.
pub struct RecoveryAttempt {
    pub latest_checkpoint: Option<Result<CheckpointRecord, CheckpointError>>,
    pub transcript_state: TranscriptState,
    pub checkpoint_write_state: CheckpointWriteState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Single documented row of the recovery precedence matrix.
pub struct RecoveryMatrixRow {
    pub latest_checkpoint_present: HasLatestCheckpoint,
    pub transcript_state: TranscriptState,
    pub checkpoint_write_state: CheckpointWriteState,
    pub result: Result<RecoveryOutcome, RecoveryError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Restart matrix state for whether a prior checkpoint write failed.
pub enum CheckpointWriteState {
    Clean,
    PriorWriteError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Policy gate result for stage-boundary checkpoint write eligibility.
pub enum StageBoundaryCheckpointPolicy {
    Write,
    Suppress,
}

impl std::ops::Not for StageBoundaryCheckpointPolicy {
    type Output = bool;

    fn not(self) -> Self::Output {
        !matches!(self, Self::Write)
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Inputs required to enforce stage-boundary checkpoint write policy and persistence.
pub struct StageBoundaryCheckpointWriteRequest {
    pub event: StageEvent,
    pub snapshot: SessionSnapshot,
    pub estimate: BudgetEstimate,
    pub payload: CheckpointPayload,
    pub config: CompactionConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Session-scoped request wrapper for restart recovery execution.
pub struct SessionRecoveryRequest {
    pub session_type: SessionType,
    pub attempt: RecoveryAttempt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Final compaction outcome with snapshot for downstream response policy.
pub struct CompactionOutcome {
    pub outcome: OutcomeKind,
    pub snapshot: SessionSnapshot,
}

/// Validate compaction configuration guardrails.
///
/// Preconditions: `request_kind` is the caller's intended request scope.
/// Postconditions: returns the unchanged config when in-range and in-scope.
/// Fails with: [`ConfigError::InvalidRatio`], [`ConfigError::InvalidIntegerField`], [`ConfigError::RewindOutOfScope`].
pub fn validate_config_guardrails(
    config: CompactionConfig,
    request_kind: RequestKind,
) -> Result<CompactionConfig, ConfigError> {
    reject_rewind_request(request_kind)?;

    if !(0.0..1.0).contains(&*config.context_budget_ratio) {
        return Err(ConfigError::InvalidRatio);
    }

    validate_positive_integer_guardrails(&config)?;

    Ok(config)
}

fn reject_rewind_request(request_kind: RequestKind) -> Result<(), ConfigError> {
    if matches!(request_kind, RequestKind::Rewind) {
        return Err(ConfigError::RewindOutOfScope);
    }
    Ok(())
}

fn validate_positive_integer_guardrails(config: &CompactionConfig) -> Result<(), ConfigError> {
    validate_non_zero_field(*config.content_clear_window, "content_clear_window")?;
    validate_non_zero_field(*config.drop_protection_window, "drop_protection_window")?;
    validate_non_zero_field(
        *config.checkpoint_summary_max_tokens,
        "checkpoint_summary_max_tokens",
    )
}

fn validate_non_zero_field(value: u32, field_name: &str) -> Result<(), ConfigError> {
    if value == 0 {
        return Err(ConfigError::InvalidIntegerField(field_name.to_owned()));
    }
    Ok(())
}

/// Seed the initial context-budget estimate for a session snapshot.
///
/// Preconditions: `snapshot` and `config` come from validated domain state.
/// Postconditions: estimate uses provider prompt tokens when present, otherwise a deterministic local estimator.
pub fn seed_budget_estimate(snapshot: SessionSnapshot, config: CompactionConfig) -> BudgetEstimate {
    let context_budget_tokens = TokenCount::new(
        (((u32::from(snapshot.model_context_limit.get())) as f64) * *config.context_budget_ratio)
            .floor() as u32,
    );
    let estimated_prompt_tokens = snapshot.provider_prompt_tokens.unwrap_or_else(|| {
        let stable_prefix_chars = snapshot.stable_prefix.bytes.chars().count() as u32;
        let turn_chars = snapshot
            .turn_pairs
            .iter()
            .map(|turn| {
                turn.user_message.body.chars().count() as u32
                    + turn.assistant_message.body.chars().count() as u32
            })
            .sum::<u32>();
        TokenCount::new(stable_prefix_chars + turn_chars)
    });

    BudgetEstimate {
        estimated_prompt_tokens,
        context_budget_tokens,
    }
}

/// Execute deterministic Stage1→Stage2→optional Stage3 compaction.
///
/// Preconditions: `snapshot` is decodable and `config` is valid for normal requests.
/// Postconditions: stable prefix bytes are preserved and returned outcome identifier is deterministic.
/// Fails with: [`CompactionError`] only for contract-level failures (for example stage-transition violations).
pub fn run_compaction_pipeline(
    snapshot: SessionSnapshot,
    config: CompactionConfig,
) -> Result<CompactionOutcome, CompactionError> {
    run_compaction_pipeline_impl(snapshot, config)
}

enum CompactionPipelineStep {
    Completed(CompactionOutcome),
    ContinueWithStage3(Box<Stage3Context>),
}

fn run_compaction_pipeline_impl(
    snapshot: SessionSnapshot,
    config: CompactionConfig,
) -> Result<CompactionOutcome, CompactionError> {
    validate_config_guardrails(config, RequestKind::Normal)
        .map_err(|_| CompactionError::ContextOverflowError)?;

    let estimate = seed_budget_estimate(snapshot.clone(), config);
    if let Some(outcome) = proceed_without_compaction_if_within_limit(&snapshot, estimate) {
        return Ok(outcome);
    }

    let stable_prefix_before = stable_prefix_bytes(&snapshot);
    let mut run = initialize_compaction_run(&snapshot)?;
    let next_step = prepare_next_pipeline_step(
        &mut run,
        CompactionPipelineContext::new(CompactionPipelineContextInit {
            snapshot: snapshot.clone(),
            stage1_snapshot: snapshot,
            config,
            context_budget_tokens: estimate.context_budget_tokens,
            stable_prefix_before: StablePrefix {
                bytes: stable_prefix_before,
            },
        }),
    )?;
    finalize_compaction_pipeline_step(&mut run, next_step)
}

fn initialize_compaction_run(snapshot: &SessionSnapshot) -> Result<CompactionRun, CompactionError> {
    let mut run = CompactionRun::new(snapshot.session_id.clone());
    run.stage1_done()
        .map_err(|_| CompactionError::ContextOverflowError)?;
    Ok(run)
}

fn stable_prefix_bytes(snapshot: &SessionSnapshot) -> String {
    snapshot.stable_prefix.bytes.clone()
}

fn finalize_compaction_pipeline_step(
    run: &mut CompactionRun,
    next_step: CompactionPipelineStep,
) -> Result<CompactionOutcome, CompactionError> {
    match next_step {
        CompactionPipelineStep::Completed(outcome) => Ok(outcome),
        CompactionPipelineStep::ContinueWithStage3(context) => {
            run_stage3_and_finalize(run, *context)
        }
    }
}

fn prepare_next_pipeline_step(
    run: &mut CompactionRun,
    context: CompactionPipelineContext,
) -> Result<CompactionPipelineStep, CompactionError> {
    prepare_next_pipeline_step_impl(run, &context)
}

fn prepare_next_pipeline_step_impl(
    run: &mut CompactionRun,
    context: &CompactionPipelineContext,
) -> Result<CompactionPipelineStep, CompactionError> {
    let stage1 = run_stage1_content_clearing(context.snapshot.clone(), context.config);
    let stage1_snapshot = stage1.snapshot.clone();
    if let Some(outcome) =
        complete_stage1_if_within_limit(run, &stage1_snapshot, context.context_budget_tokens)?
    {
        return Ok(CompactionPipelineStep::Completed(outcome));
    }
    run.stage2_done()
        .map_err(|_| CompactionError::ContextOverflowError)?;

    // Update context for stage2
    let stage2_context = CompactionPipelineContext::new(CompactionPipelineContextInit {
        snapshot: context.snapshot.clone(),
        stage1_snapshot,
        config: context.config,
        context_budget_tokens: context.context_budget_tokens,
        stable_prefix_before: context.stable_prefix_before.clone(),
    });
    prepare_stage2_pipeline_step(run, &stage2_context)
}

fn complete_stage1_if_within_limit(
    run: &mut CompactionRun,
    stage1_snapshot: &SessionSnapshot,
    context_budget_tokens: TokenCount,
) -> Result<Option<CompactionOutcome>, CompactionError> {
    complete_stage_if_within_limit(
        run,
        StageCompletionCheck {
            snapshot: stage1_snapshot.clone(),
            snapshot_for_estimate: stage1_snapshot.clone(),
            context_budget_tokens,
            reason: CompactionCompletionReason::Stage1WithinBudget,
        },
    )
}

enum Stage2PipelineStep {
    Completed(CompactionOutcome),
    Continue {
        stage2: Stage2Result,
        stage2_snapshot: SessionSnapshot,
    },
}

struct Stage2PipelineContext {
    stage1_snapshot: SessionSnapshot,
    config: CompactionConfig,
    context_budget_tokens: TokenCount,
}

fn run_stage2_and_maybe_complete(
    run: &mut CompactionRun,
    context: Stage2PipelineContext,
) -> Result<Stage2PipelineStep, CompactionError> {
    let (stage2, stage2_snapshot) = run_stage2(context.stage1_snapshot, context.config);
    if let Some(outcome) = complete_stage_if_within_limit(
        run,
        StageCompletionCheck {
            snapshot: stage2_snapshot.clone(),
            snapshot_for_estimate: stage2_snapshot.clone(),
            context_budget_tokens: context.context_budget_tokens,
            reason: CompactionCompletionReason::Stage2WithinBudget,
        },
    )? {
        return Ok(Stage2PipelineStep::Completed(outcome));
    }
    Ok(Stage2PipelineStep::Continue {
        stage2,
        stage2_snapshot,
    })
}

fn prepare_stage2_pipeline_step(
    run: &mut CompactionRun,
    context: &CompactionPipelineContext,
) -> Result<CompactionPipelineStep, CompactionError> {
    match run_stage2_and_maybe_complete(
        run,
        Stage2PipelineContext {
            stage1_snapshot: context.stage1_snapshot.clone(),
            config: context.config,
            context_budget_tokens: context.context_budget_tokens,
        },
    )? {
        Stage2PipelineStep::Completed(outcome) => Ok(CompactionPipelineStep::Completed(outcome)),
        Stage2PipelineStep::Continue {
            stage2,
            stage2_snapshot,
        } => Ok(CompactionPipelineStep::ContinueWithStage3(Box::new(
            Stage3Context {
                snapshots: Stage3Snapshots {
                    snapshot: context.snapshot.clone(),
                    stage1_snapshot: context.stage1_snapshot.clone(),
                    stage2_snapshot,
                },
                stage2,
                policy: Stage3Policy {
                    context_budget_tokens: context.context_budget_tokens,
                    stable_prefix_before: context.stable_prefix_before.bytes.clone(),
                    config: context.config,
                },
            },
        ))),
    }
}

struct StageCompletionCheck {
    snapshot: SessionSnapshot,
    snapshot_for_estimate: SessionSnapshot,
    context_budget_tokens: TokenCount,
    reason: CompactionCompletionReason,
}

fn complete_stage_if_within_limit(
    run: &mut CompactionRun,
    check: StageCompletionCheck,
) -> Result<Option<CompactionOutcome>, CompactionError> {
    complete_if_within_limit(
        run,
        CompletionWithinLimit {
            snapshot: check.snapshot,
            estimate: estimate_snapshot_with_budget(
                &check.snapshot_for_estimate,
                check.context_budget_tokens,
            ),
            reason: check.reason,
        },
    )
}

fn proceed_without_compaction_if_within_limit(
    snapshot: &SessionSnapshot,
    estimate: BudgetEstimate,
) -> Option<CompactionOutcome> {
    budget_within_limit(estimate).then_some(CompactionOutcome {
        outcome: OutcomeKind::ProceedWithoutCompaction,
        snapshot: snapshot.clone(),
    })
}

fn complete_if_within_limit(
    run: &mut CompactionRun,
    completion: CompletionWithinLimit,
) -> Result<Option<CompactionOutcome>, CompactionError> {
    if !budget_within_limit(completion.estimate) {
        return Ok(None);
    }
    run.complete(completion.reason)
        .map_err(|_| CompactionError::ContextOverflowError)?;
    Ok(Some(proceed_without_stage3(completion.snapshot)))
}

struct CompletionWithinLimit {
    snapshot: SessionSnapshot,
    estimate: BudgetEstimate,
    reason: CompactionCompletionReason,
}

fn proceed_without_stage3(snapshot: SessionSnapshot) -> CompactionOutcome {
    CompactionOutcome {
        outcome: OutcomeKind::ProceedWithoutStage3,
        snapshot,
    }
}

fn budget_within_limit(estimate: BudgetEstimate) -> bool {
    u32::from(estimate.estimated_prompt_tokens.get())
        <= u32::from(estimate.context_budget_tokens.get())
}

fn estimate_snapshot_with_budget(
    snapshot: &SessionSnapshot,
    context_budget_tokens: TokenCount,
) -> BudgetEstimate {
    BudgetEstimate {
        estimated_prompt_tokens: TokenCount::new(estimate_snapshot_tokens(snapshot)),
        context_budget_tokens,
    }
}

fn run_stage2(
    snapshot: SessionSnapshot,
    config: CompactionConfig,
) -> (Stage2Result, SessionSnapshot) {
    let stage2_candidates = classify_stage2_candidates(snapshot.clone(), config);
    let stage2 = score_and_drop_stage2_candidates(stage2_candidates, config);
    let mut stage2_snapshot = snapshot;
    stage2_snapshot
        .turn_pairs
        .retain(|turn| !stage2.dropped_turn_ids.contains(&turn.id));
    (stage2, stage2_snapshot)
}

struct Stage3Snapshots {
    snapshot: SessionSnapshot,
    stage1_snapshot: SessionSnapshot,
    stage2_snapshot: SessionSnapshot,
}

struct Stage3Policy {
    context_budget_tokens: TokenCount,
    stable_prefix_before: String,
    config: CompactionConfig,
}

struct Stage3Context {
    snapshots: Stage3Snapshots,
    stage2: Stage2Result,
    policy: Stage3Policy,
}

struct Stage3WorkItem {
    lease_token: LeaseToken,
    segment: DroppableSegment,
}

struct Stage3Completion {
    work_item: Stage3WorkItem,
    summary: SummaryBlock,
}

fn run_stage3_and_finalize(
    run: &mut CompactionRun,
    context: Stage3Context,
) -> Result<CompactionOutcome, CompactionError> {
    if let Some(outcome) = background_pressure_outcome(run, &context)? {
        return Ok(outcome);
    }

    let completion = match build_stage3_completion(run, &context) {
        Ok(completion) => completion,
        Err(Stage3BuildFailure::Outcome(outcome)) => return Ok(outcome),
        Err(Stage3BuildFailure::Error(err)) => return Err(err),
    };

    finalize_stage3_success(run, context, completion)
}

enum Stage3BuildFailure {
    Outcome(CompactionOutcome),
    Error(CompactionError),
}

fn build_stage3_completion(
    run: &mut CompactionRun,
    context: &Stage3Context,
) -> Result<Stage3Completion, Stage3BuildFailure> {
    let segment = resolve_droppable_segment(run, context)?;
    let lease_token = acquire_stage3_lease_or_warn(run, context)?;
    let work_item = Stage3WorkItem {
        lease_token,
        segment,
    };
    let summary = generate_and_validate_summary(run, context, &work_item)?;
    Ok(Stage3Completion { work_item, summary })
}

fn background_pressure_outcome(
    run: &mut CompactionRun,
    context: &Stage3Context,
) -> Result<Option<CompactionOutcome>, CompactionError> {
    if !matches!(
        context.snapshots.snapshot.session_type,
        SessionType::Background
    ) {
        return Ok(None);
    }
    complete_run(run, CompactionCompletionReason::BackgroundPressure)?;
    Ok(Some(CompactionOutcome {
        outcome: OutcomeKind::ContextPressureWarning,
        snapshot: context.snapshots.stage2_snapshot.clone(),
    }))
}

fn resolve_droppable_segment(
    run: &mut CompactionRun,
    context: &Stage3Context,
) -> Result<DroppableSegment, Stage3BuildFailure> {
    match compute_droppable_segment(
        context.snapshots.stage1_snapshot.clone(),
        context.stage2.clone(),
        context.policy.config,
    ) {
        Ok(segment) => Ok(segment),
        Err(_) => {
            complete_run(run, CompactionCompletionReason::EmptyDroppableSegment)
                .map_err(Stage3BuildFailure::Error)?;
            Err(Stage3BuildFailure::Outcome(CompactionOutcome {
                outcome: OutcomeKind::ContextOverflowError,
                snapshot: context.snapshots.stage2_snapshot.clone(),
            }))
        }
    }
}

fn acquire_stage3_lease_or_warn(
    run: &mut CompactionRun,
    context: &Stage3Context,
) -> Result<LeaseToken, Stage3BuildFailure> {
    let global_window = WindowId::new("global")
        .map_err(|_| Stage3BuildFailure::Error(CompactionError::LeaseDenied))?;
    match try_acquire_rate_slot_lease(global_window, *context.policy.config.rate_budget_reserve) {
        LeaseDecision::Granted(token) => {
            mark_stage3_pending(run).map_err(Stage3BuildFailure::Error)?;
            Ok(token)
        }
        LeaseDecision::Denied(_) => {
            complete_run(run, CompactionCompletionReason::LeaseDenied)
                .map_err(Stage3BuildFailure::Error)?;
            Err(Stage3BuildFailure::Outcome(CompactionOutcome {
                outcome: OutcomeKind::ContextPressureWarning,
                snapshot: context.snapshots.stage2_snapshot.clone(),
            }))
        }
    }
}

fn generate_and_validate_summary(
    run: &mut CompactionRun,
    context: &Stage3Context,
    work_item: &Stage3WorkItem,
) -> Result<SummaryBlock, Stage3BuildFailure> {
    let preserved = PreservationSet {
        required_elements: vec!["objective".to_owned()],
    };
    let summary = generate_stage3_summary(SummaryRequest {
        segment: work_item.segment.clone(),
        preservation_set: preserved.clone(),
    })
    .map_err(|_| {
        match summary_generation_error_outcome(run, context, work_item.lease_token.clone()) {
            Ok(outcome) => Stage3BuildFailure::Outcome(outcome),
            Err(err) => Stage3BuildFailure::Error(err),
        }
    })?;
    validate_summary_contract(summary, work_item.segment.clone(), preserved).map_err(|_| {
        match summary_contract_error_outcome(run, context, work_item.lease_token.clone()) {
            Ok(outcome) => Stage3BuildFailure::Outcome(outcome),
            Err(err) => Stage3BuildFailure::Error(err),
        }
    })
}

fn summary_generation_error_outcome(
    run: &mut CompactionRun,
    context: &Stage3Context,
    lease_token: LeaseToken,
) -> Result<CompactionOutcome, CompactionError> {
    let _ = consume_rate_slot_lease(lease_token, LeaseConsumeReason::Failed);
    complete_run(run, CompactionCompletionReason::SummaryGenerationFailed)?;
    Ok(CompactionOutcome {
        outcome: OutcomeKind::SummaryGenerationError,
        snapshot: context.snapshots.stage2_snapshot.clone(),
    })
}

fn summary_contract_error_outcome(
    run: &mut CompactionRun,
    context: &Stage3Context,
    lease_token: LeaseToken,
) -> Result<CompactionOutcome, CompactionError> {
    let _ = consume_rate_slot_lease(lease_token, LeaseConsumeReason::Failed);
    complete_run(run, CompactionCompletionReason::SummaryContractFailed)?;
    Ok(CompactionOutcome {
        outcome: OutcomeKind::ContextOverflowError,
        snapshot: context.snapshots.stage2_snapshot.clone(),
    })
}

fn complete_run(
    run: &mut CompactionRun,
    reason: CompactionCompletionReason,
) -> Result<(), CompactionError> {
    run.complete(reason)
        .map_err(|_| CompactionError::ContextOverflowError)
}

fn mark_stage3_pending(run: &mut CompactionRun) -> Result<(), CompactionError> {
    run.stage3_pending()
        .map_err(|_| CompactionError::ContextOverflowError)
}

fn finalize_stage3_success(
    run: &mut CompactionRun,
    context: Stage3Context,
    completion: Stage3Completion,
) -> Result<CompactionOutcome, CompactionError> {
    finalize_stage3_success_impl(run, context, completion)
}

fn finalize_stage3_success_impl(
    run: &mut CompactionRun,
    context: Stage3Context,
    completion: Stage3Completion,
) -> Result<CompactionOutcome, CompactionError> {
    let committed_snapshot = commit_stage3_summary(&context, completion)?;
    if let Some(outcome) = complete_stage3_or_return_overflow(run, &context, &committed_snapshot)? {
        return Ok(outcome);
    }
    run.complete(CompactionCompletionReason::SummaryCommitted)
        .map_err(|_| CompactionError::ContextOverflowError)?;
    Ok(CompactionOutcome {
        outcome: OutcomeKind::ProceedWithSummary,
        snapshot: committed_snapshot,
    })
}

fn commit_stage3_summary(
    context: &Stage3Context,
    completion: Stage3Completion,
) -> Result<SessionSnapshot, CompactionError> {
    let committed_snapshot = commit_summary_replacement(
        context.snapshots.stage2_snapshot.clone(),
        completion.work_item.segment,
        completion.summary,
    )?;
    let _ = consume_rate_slot_lease(completion.work_item.lease_token, LeaseConsumeReason::Used);
    Ok(committed_snapshot)
}

fn complete_stage3_or_return_overflow(
    run: &mut CompactionRun,
    context: &Stage3Context,
    committed_snapshot: &SessionSnapshot,
) -> Result<Option<CompactionOutcome>, CompactionError> {
    if let Some(outcome) = stage3_final_budget_overflow_outcome(run, context, committed_snapshot)? {
        return Ok(Some(outcome));
    }
    ensure_stable_prefix_unchanged(committed_snapshot, context)?;
    Ok(None)
}

fn stage3_final_budget_overflow_outcome(
    run: &mut CompactionRun,
    context: &Stage3Context,
    committed_snapshot: &SessionSnapshot,
) -> Result<Option<CompactionOutcome>, CompactionError> {
    let final_estimate =
        estimate_snapshot_with_budget(committed_snapshot, context.policy.context_budget_tokens);
    if budget_within_limit(final_estimate) {
        return Ok(None);
    }
    run.complete(CompactionCompletionReason::FinalBudgetOverflow)
        .map_err(|_| CompactionError::ContextOverflowError)?;
    Ok(Some(CompactionOutcome {
        outcome: OutcomeKind::ContextOverflowError,
        snapshot: context.snapshots.stage2_snapshot.clone(),
    }))
}

fn ensure_stable_prefix_unchanged(
    committed_snapshot: &SessionSnapshot,
    context: &Stage3Context,
) -> Result<(), CompactionError> {
    if committed_snapshot.stable_prefix.bytes != context.policy.stable_prefix_before {
        return Err(CompactionError::InvalidSummaryContract);
    }
    Ok(())
}

/// Run Stage 1 content clearing over eligible historical turn-pair bodies.
///
/// Preconditions: turn ages are indexed.
/// Postconditions: only tool-result bodies on turns with `age > content_clear_window`
/// and not excluded-from-clearing are body-cleared.
pub fn run_stage1_content_clearing(
    snapshot: SessionSnapshot,
    config: CompactionConfig,
) -> Stage1Result {
    let mut updated = snapshot;

    for turn in &mut updated.turn_pairs {
        clear_turn_content_if_eligible(turn, &config);
    }

    Stage1Result { snapshot: updated }
}

fn clear_turn_content_if_eligible(turn: &mut TurnPair, config: &CompactionConfig) {
    let should_clear = u32::from(turn.age.get()) > *config.content_clear_window
        && !turn.metadata.excluded_from_clearing.0;
    if !should_clear {
        return;
    }
    clear_tool_result_message(&mut turn.user_message);
    clear_tool_result_message(&mut turn.assistant_message);
}

fn clear_tool_result_message(message: &mut Message) {
    if message.is_tool_result.0 {
        message.body = OutputText::from(CLEAR_MARKER);
    }
}

/// Classify Stage 2 drop candidates into one mutually-exclusive class per eligible turn.
pub fn classify_stage2_candidates(
    snapshot: SessionSnapshot,
    _config: CompactionConfig,
) -> Vec<ClassifiedCandidate> {
    let mut candidates = Vec::new();

    for turn in snapshot.turn_pairs {
        if should_skip_stage2_candidate(&turn) {
            continue;
        }
        candidates.push(ClassifiedCandidate {
            turn_id: turn.id,
            age: turn.age,
            class: classify_stage2_candidate(&turn),
        });
    }

    candidates
}

fn should_skip_stage2_candidate(turn: &TurnPair) -> bool {
    turn.metadata.protected_recent_window.0 || turn.metadata.objective_changing.0
}

fn classify_stage2_candidate(turn: &TurnPair) -> CandidateClass {
    if turn.user_message.is_tool_result.0 && turn.assistant_message.is_tool_result.0 {
        return CandidateClass::PureToolExchange;
    }
    if stage2_turn_is_cleared_or_empty(turn) {
        return CandidateClass::ClearedEmpty;
    }
    CandidateClass::LowSemanticDensity
}

fn stage2_turn_is_cleared_or_empty(turn: &TurnPair) -> bool {
    turn.user_message.body.is_empty()
        || turn.assistant_message.body.is_empty()
        || turn.user_message.body == CLEAR_MARKER
        || turn.assistant_message.body == CLEAR_MARKER
}

/// Score classified Stage 2 candidates and order drop IDs by score then age.
pub fn score_and_drop_stage2_candidates(
    mut candidates: Vec<ClassifiedCandidate>,
    _config: CompactionConfig,
) -> Stage2Result {
    candidates.sort_by(|a, b| {
        let score_a = candidate_score(&a.class);
        let score_b = candidate_score(&b.class);
        score_a
            .cmp(&score_b)
            .then_with(|| u32::from(b.age.get()).cmp(&u32::from(a.age.get())))
    });

    Stage2Result {
        dropped_turn_ids: candidates.into_iter().map(|c| c.turn_id).collect(),
    }
}

/// Compute the contiguous droppable segment for Stage 3 summarization.
///
/// Fails with: [`CompactionError::EmptyDroppableSegment`] when no safe droppable turn IDs remain.
pub fn compute_droppable_segment(
    snapshot: SessionSnapshot,
    stage2: Stage2Result,
    _config: CompactionConfig,
) -> Result<DroppableSegment, CompactionError> {
    if stage2.dropped_turn_ids.is_empty() {
        return Err(CompactionError::EmptyDroppableSegment);
    }

    let candidate_indices = collect_droppable_candidate_indices(&snapshot, &stage2);
    let contiguous = leading_contiguous_indices(candidate_indices);
    let turn_ids = segment_turn_ids_from_indices(&snapshot, contiguous);
    droppable_segment_from_turn_ids(turn_ids)
}

fn collect_droppable_candidate_indices(
    snapshot: &SessionSnapshot,
    stage2: &Stage2Result,
) -> Vec<usize> {
    let dropped_set: HashSet<TurnPairId> = stage2.dropped_turn_ids.iter().copied().collect();
    snapshot
        .turn_pairs
        .iter()
        .enumerate()
        .filter(|(_, turn)| {
            dropped_set.contains(&turn.id)
                && !turn.metadata.protected_recent_window
                && !turn.metadata.objective_changing
        })
        .map(|(idx, _)| idx)
        .collect()
}

fn leading_contiguous_indices(candidate_indices: Vec<usize>) -> Vec<usize> {
    let Some(first) = candidate_indices.first().copied() else {
        return Vec::new();
    };
    let mut contiguous = vec![first];
    for idx in candidate_indices.into_iter().skip(1) {
        let expected_next = contiguous.last().copied().unwrap_or(idx) + 1;
        if idx != expected_next {
            break;
        }
        contiguous.push(idx);
    }
    contiguous
}

fn segment_turn_ids_from_indices(
    snapshot: &SessionSnapshot,
    indices: Vec<usize>,
) -> Vec<TurnPairId> {
    indices
        .into_iter()
        .map(|idx| snapshot.turn_pairs[idx].id)
        .collect()
}

fn droppable_segment_from_turn_ids(
    turn_ids: Vec<TurnPairId>,
) -> Result<DroppableSegment, CompactionError> {
    let Some(start_turn) = turn_ids.first().copied() else {
        return Err(CompactionError::EmptyDroppableSegment);
    };
    let Some(end_turn) = turn_ids.last().copied() else {
        return Err(CompactionError::EmptyDroppableSegment);
    };
    Ok(DroppableSegment {
        start_turn,
        end_turn,
        turn_ids,
    })
}

/// Attempt an atomic Stage 3 rate-slot reservation for the provided window.
///
/// Preconditions: `reserve` is an unsigned integer boundary reserve.
/// Postconditions: at most one winner exists per available slot boundary.
pub fn try_acquire_rate_slot_lease(
    window_id: WindowId,
    reserve: impl Into<RateSlotReserve>,
) -> LeaseDecision {
    let reserve_slots: u32 = reserve.into().into();
    if reserve_slots >= TOTAL_RATE_SLOTS {
        return LeaseDecision::Denied(LeaseDenyReason::ReserveExhausted);
    }

    let allowed = TOTAL_RATE_SLOTS - reserve_slots;
    let mut state = LEASE_STATE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let reserved = state.entry(window_id.as_str().to_owned()).or_insert(0);
    if *reserved >= allowed {
        LeaseDecision::Denied(LeaseDenyReason::SlotUnavailable)
    } else {
        *reserved += 1;
        let mut counter = LEASE_ISSUE_COUNTER
            .get_or_init(|| Mutex::new(0))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *counter += 1;
        let token = match LeaseToken::new(format!("{}:{}", window_id.as_str(), *counter)) {
            Ok(token) => token,
            Err(_) => {
                *reserved = reserved.saturating_sub(1);
                return LeaseDecision::Denied(LeaseDenyReason::TokenGenerationFailed);
            }
        };
        let mut records = LEASE_RECORDS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        records.insert(token.as_str().to_owned(), RateLeaseLifecycle::Reserved);
        LeaseDecision::Granted(token)
    }
}

fn release_window_slot(token: &LeaseToken) {
    let Some((window, _)) = token.as_str().split_once(':') else {
        return;
    };
    let mut state = LEASE_STATE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(reserved) = state.get_mut(window) {
        *reserved = reserved.saturating_sub(1);
    }
}

/// Consume or expire an acquired lease token.
///
/// Postconditions: acquired leases become terminal (`Consumed`/`Expired`) and cannot be consumed twice.
pub fn consume_rate_slot_lease(
    lease: LeaseToken,
    status: LeaseConsumeReason,
) -> LeaseConsumeResult {
    if lease.as_str().is_empty() {
        return LeaseConsumeResult::UnknownLease;
    }

    if matches!(status, LeaseConsumeReason::Expired) {
        return expire_rate_slot_lease(lease);
    }

    consume_reserved_rate_slot_lease(lease, status)
}

fn consume_reserved_rate_slot_lease(
    lease: LeaseToken,
    status: LeaseConsumeReason,
) -> LeaseConsumeResult {
    let mut records = LEASE_RECORDS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let Some(lifecycle) = records.get_mut(lease.as_str()) else {
        return LeaseConsumeResult::UnknownLease;
    };

    match lifecycle {
        RateLeaseLifecycle::Reserved => {
            *lifecycle = RateLeaseLifecycle::Consumed(status);
            drop(records);
            release_window_slot(&lease);
            LeaseConsumeResult::Consumed
        }
        RateLeaseLifecycle::Consumed(_) | RateLeaseLifecycle::Expired => {
            LeaseConsumeResult::AlreadyConsumed
        }
        RateLeaseLifecycle::Available => LeaseConsumeResult::UnknownLease,
    }
}

fn expire_rate_slot_lease(lease: LeaseToken) -> LeaseConsumeResult {
    if lease.as_str().is_empty() {
        return LeaseConsumeResult::UnknownLease;
    }

    let mut records = LEASE_RECORDS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let Some(lifecycle) = records.get_mut(lease.as_str()) else {
        return LeaseConsumeResult::UnknownLease;
    };

    match lifecycle {
        RateLeaseLifecycle::Reserved => {
            *lifecycle = RateLeaseLifecycle::Expired;
            drop(records);
            release_window_slot(&lease);
            LeaseConsumeResult::Consumed
        }
        RateLeaseLifecycle::Consumed(_) | RateLeaseLifecycle::Expired => {
            LeaseConsumeResult::AlreadyConsumed
        }
        RateLeaseLifecycle::Available => LeaseConsumeResult::UnknownLease,
    }
}

/// Generate a Stage 3 summary block request payload.
///
/// Preconditions: the droppable segment is non-empty.
/// Postconditions: summary is tagged as a compaction summary and uses canonical segment header.
/// Fails with: [`CompactionError::SummaryGenerationError`] when generation preconditions are not met.
pub fn generate_stage3_summary(request: SummaryRequest) -> Result<SummaryBlock, CompactionError> {
    if request.segment.turn_ids.is_empty() {
        return Err(CompactionError::SummaryGenerationError);
    }

    let body = if request.preservation_set.required_elements.is_empty() {
        "A dense narrative summary of earlier context is provided for continuity.".to_owned()
    } else {
        format!(
            "A dense narrative summary preserves {} while replacing older droppable turns.",
            request.preservation_set.required_elements.join(", ")
        )
    };

    Ok(SummaryBlock {
        header: canonical_summary_header(&request.segment),
        body,
        compaction_summary: IsCompactionSummary::yes(),
    })
}

/// Validate the Stage 3 summary contract before replacement commit.
///
/// Preconditions: summary has header/body, non-empty segment, and non-empty preservation set.
/// Postconditions: header is canonical for the segment, body is dense prose, and estimated size is <= 500 tokens.
/// Fails with: [`CompactionError::InvalidSummaryContract`].
pub fn validate_summary_contract(
    summary: SummaryBlock,
    segment: DroppableSegment,
    preservation_set: PreservationSet,
) -> Result<SummaryBlock, CompactionError> {
    ensure_summary_shape(&summary, &segment)?;
    let normalized_body = ensure_summary_body_format(&summary)?;
    ensure_preservation_requirements(&normalized_body, &preservation_set)?;
    Ok(summary)
}

fn ensure_summary_shape(
    summary: &SummaryBlock,
    segment: &DroppableSegment,
) -> Result<(), CompactionError> {
    let expected_header = canonical_summary_header(segment);
    if summary.header != expected_header || segment.turn_ids.is_empty() {
        return Err(CompactionError::InvalidSummaryContract);
    }
    if !summary.compaction_summary.0 || summary.body.trim().is_empty() {
        return Err(CompactionError::InvalidSummaryContract);
    }
    Ok(())
}

fn ensure_summary_body_format(summary: &SummaryBlock) -> Result<String, CompactionError> {
    let normalized_body = normalize_lf(&summary.body);
    reject_forbidden_summary_content(&normalized_body, summary)?;
    reject_summary_markdown_list_lines(&normalized_body)?;
    reject_oversized_summary_body(&normalized_body)?;
    Ok(normalized_body)
}

fn reject_forbidden_summary_content(
    normalized_body: &str,
    summary: &SummaryBlock,
) -> Result<(), CompactionError> {
    if normalized_body.contains("```") || normalized_body.contains(&summary.header) {
        return Err(CompactionError::InvalidSummaryContract);
    }
    Ok(())
}

fn reject_summary_markdown_list_lines(normalized_body: &str) -> Result<(), CompactionError> {
    if normalized_body.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with('-') || trimmed.starts_with('*') || trimmed.starts_with('#')
    }) {
        return Err(CompactionError::InvalidSummaryContract);
    }
    Ok(())
}

fn reject_oversized_summary_body(normalized_body: &str) -> Result<(), CompactionError> {
    if normalized_body.split_whitespace().count() as u32
        > u32::from(SUMMARY_BODY_MAX_ESTIMATED_TOKENS)
    {
        return Err(CompactionError::InvalidSummaryContract);
    }
    Ok(())
}

fn ensure_preservation_requirements(
    normalized_body: &str,
    preservation_set: &PreservationSet,
) -> Result<(), CompactionError> {
    if preservation_set.required_elements.is_empty() {
        return Err(CompactionError::InvalidSummaryContract);
    }
    let body_lower = normalized_body.to_lowercase();
    let missing_required = preservation_set
        .required_elements
        .iter()
        .any(|required| !body_lower.contains(&required.to_lowercase()));
    if missing_required {
        return Err(CompactionError::InvalidSummaryContract);
    }
    Ok(())
}

/// Commit summary replacement for the droppable segment while preserving protected turns.
///
/// Preconditions: segment is non-empty and excludes protected/objective-changing turns.
/// Postconditions: segment is replaced in-place with one compaction-summary turn and stable prefix is unchanged.
pub fn commit_summary_replacement(
    snapshot: SessionSnapshot,
    segment: DroppableSegment,
    summary: SummaryBlock,
) -> Result<SessionSnapshot, CompactionError> {
    if segment.turn_ids.is_empty() {
        return Err(CompactionError::EmptyDroppableSegment);
    }
    if segment_contains_protected_or_objective_turn(&snapshot, &segment) {
        return Err(CompactionError::InvalidSummaryContract);
    }

    let (updated_pairs, inserted_summary) =
        replace_segment_with_summary(snapshot.turn_pairs, segment, summary);
    if !inserted_summary {
        return Err(CompactionError::EmptyDroppableSegment);
    }

    Ok(SessionSnapshot {
        turn_pairs: updated_pairs,
        ..snapshot
    })
}

fn segment_contains_protected_or_objective_turn(
    snapshot: &SessionSnapshot,
    segment: &DroppableSegment,
) -> bool {
    snapshot.turn_pairs.iter().any(|turn| {
        segment.turn_ids.contains(&turn.id)
            && (turn.metadata.protected_recent_window.0 || turn.metadata.objective_changing.0)
    })
}

fn replace_segment_with_summary(
    turn_pairs: Vec<TurnPair>,
    segment: DroppableSegment,
    summary: SummaryBlock,
) -> (Vec<TurnPair>, bool) {
    let mut updated_pairs = Vec::with_capacity(turn_pairs.len());
    let drop_set: HashSet<TurnPairId> = segment.turn_ids.into_iter().collect();
    let mut inserted_summary = false;

    for turn in turn_pairs {
        if drop_set.contains(&turn.id) {
            if !inserted_summary {
                updated_pairs.push(build_summary_turn(&turn, &summary));
                inserted_summary = true;
            }
            continue;
        }
        updated_pairs.push(turn);
    }

    (updated_pairs, inserted_summary)
}

fn build_summary_turn(source_turn: &TurnPair, summary: &SummaryBlock) -> TurnPair {
    TurnPair {
        identity: TurnPairIdentity {
            id: source_turn.id,
            objective_id: source_turn.objective_id.clone(),
        },
        user_message: Message {
            body: OutputText::from("[compaction-summary]"),
            is_tool_result: IsToolResult::no(),
        },
        assistant_message: Message {
            body: OutputText::from(format!("{}\n{}", summary.header, summary.body)),
            is_tool_result: IsToolResult::no(),
        },
        age: source_turn.age,
        metadata: TurnPairMetadata {
            protected_recent_window: IsPredicate::no(),
            objective_changing: IsPredicate::no(),
            excluded_from_clearing: IsPredicate::yes(),
            low_semantic_density: IsPredicate::no(),
        },
    }
}

/// Evaluate background-session send policy from the current budget estimate.
pub fn evaluate_background_policy(
    snapshot: SessionSnapshot,
    estimate: BudgetEstimate,
) -> BackgroundPolicyDecision {
    let background_over_budget = matches!(snapshot.session_type, SessionType::Background)
        && u32::from(estimate.estimated_prompt_tokens.get())
            > u32::from(estimate.context_budget_tokens.get());

    if background_over_budget {
        BackgroundPolicyDecision {
            should_send_request: ShouldSendRequest::no(),
            outcome: OutcomeKind::ContextPressureWarning,
        }
    } else {
        BackgroundPolicyDecision {
            should_send_request: ShouldSendRequest::yes(),
            outcome: OutcomeKind::ProceedWithoutStage3,
        }
    }
}

/// Map a compaction outcome kind to its deterministic response envelope identifier.
pub fn emit_response_identifier(result: OutcomeKind) -> ResponseEnvelope {
    ResponseEnvelope {
        identifier: result.response_identifier(),
    }
}

/// Gate checkpoint writes to main-session stage-boundary events only.
pub fn should_write_stage_boundary_checkpoint(
    event: StageEvent,
    session_type: SessionType,
) -> StageBoundaryCheckpointPolicy {
    if matches!(session_type, SessionType::Main) && matches!(event, StageEvent::StageBoundary(_)) {
        return StageBoundaryCheckpointPolicy::Write;
    }
    StageBoundaryCheckpointPolicy::Suppress
}

/// Validate checkpoint payload schema and summary-size constraints.
///
/// Preconditions: payload corresponds to a stage-boundary event.
/// Postconditions: required fields are non-empty; Review stage requires `next_stage=Complete`.
/// Fails with: [`CheckpointError::PayloadSchemaError`] or [`CheckpointError::SummaryTooLarge`].
pub fn validate_checkpoint_payload(
    payload: CheckpointPayload,
    config: CompactionConfig,
) -> Result<CheckpointPayload, CheckpointError> {
    validate_checkpoint_required_fields(&payload)?;
    validate_checkpoint_stage_transition(&payload)?;
    validate_checkpoint_summary_size(&payload, config)?;
    Ok(payload)
}

fn validate_checkpoint_required_fields(payload: &CheckpointPayload) -> Result<(), CheckpointError> {
    for (text, message) in [
        (payload.objective.as_str(), "objective is required"),
        (
            payload.narrative.context_summary.as_str(),
            "context_summary is required",
        ),
    ] {
        validate_checkpoint_required_text(text, message)?;
    }
    for (entries, message) in [
        (
            payload.narrative.artifacts.as_slice(),
            "artifacts entries must be non-empty",
        ),
        (
            payload.narrative.decisions.as_slice(),
            "decisions entries must be non-empty",
        ),
        (
            payload.narrative.open_questions.as_slice(),
            "open_questions entries must be non-empty",
        ),
    ] {
        validate_checkpoint_required_entries(entries, message)?;
    }
    Ok(())
}

fn validate_checkpoint_required_text(text: &str, message: &str) -> Result<(), CheckpointError> {
    if text.trim().is_empty() {
        return Err(CheckpointError::PayloadSchemaError(message.to_owned()));
    }
    Ok(())
}

fn validate_checkpoint_required_entries(
    entries: &[String],
    message: &str,
) -> Result<(), CheckpointError> {
    if entries.iter().any(|item| item.trim().is_empty()) {
        return Err(CheckpointError::PayloadSchemaError(message.to_owned()));
    }
    Ok(())
}

fn validate_checkpoint_stage_transition(
    payload: &CheckpointPayload,
) -> Result<(), CheckpointError> {
    if matches!(payload.stage_completed, StageName::Review)
        && !matches!(payload.next_stage, StageName::Complete)
    {
        return Err(CheckpointError::PayloadSchemaError(
            "review checkpoints must set next_stage=complete".to_owned(),
        ));
    }
    Ok(())
}

fn validate_checkpoint_summary_size(
    payload: &CheckpointPayload,
    config: CompactionConfig,
) -> Result<(), CheckpointError> {
    if normalize_lf(&payload.narrative.context_summary)
        .split_whitespace()
        .count() as u32
        > *config.checkpoint_summary_max_tokens
    {
        return Err(CheckpointError::SummaryTooLarge);
    }
    Ok(())
}

/// Validate and persist a stage-boundary checkpoint candidate.
///
/// Preconditions: checkpoint policy gate has already allowed write in caller flow.
/// Postconditions: returned record lifecycle is `Persisted`.
/// Fails with: [`CheckpointError::CheckpointWriteError`] for transition or forced write failures.
pub fn write_stage_boundary_checkpoint(
    payload: CheckpointPayload,
) -> Result<CheckpointRecord, CheckpointError> {
    let candidate = CheckpointRecord::new_candidate(payload);
    let validated = candidate.transition_to(CheckpointLifecycle::Validated)?;
    if validated
        .payload
        .narrative
        .decisions
        .iter()
        .any(|decision| decision == "__force_write_error__")
    {
        let _ = validated.transition_write_failure()?;
        return Err(CheckpointError::CheckpointWriteError);
    }
    validated.transition_to(CheckpointLifecycle::Persisted)
}

/// Orchestrate stage-boundary checkpoint write with production guard enforcement.
///
/// Preconditions: caller provides session snapshot and current budget estimate.
/// Postconditions: background sessions and non-boundary events are blocked; stage
/// completion must observe successful checkpoint persistence.
/// Fails with: externally observed [`CheckpointError::CheckpointWriteError`] for
/// blocked writes or oversized summaries, plus schema/corruption failures.
pub fn orchestrate_stage_boundary_checkpoint_write(
    request: StageBoundaryCheckpointWriteRequest,
) -> Result<CheckpointRecord, CheckpointError> {
    let background_policy = evaluate_background_policy(request.snapshot.clone(), request.estimate);
    if matches!(request.snapshot.session_type, SessionType::Background)
        && !background_policy.should_send_request.0
    {
        return Err(CheckpointError::CheckpointWriteError);
    }

    if !matches!(
        should_write_stage_boundary_checkpoint(request.event, request.snapshot.session_type),
        StageBoundaryCheckpointPolicy::Write
    ) {
        return Err(CheckpointError::CheckpointWriteError);
    }

    let map_external_error = |error: CheckpointError| match error {
        CheckpointError::SummaryTooLarge => CheckpointError::CheckpointWriteError,
        other => other,
    };
    let validated =
        validate_checkpoint_payload(request.payload, request.config).map_err(map_external_error)?;
    write_stage_boundary_checkpoint(validated).map_err(map_external_error)
}

/// Select the latest checkpoint by `(checkpoint_sequence, created_at)` or fail closed as corruption.
pub fn select_latest_checkpoint_or_corruption(
    index: Vec<CheckpointRecord>,
) -> Result<CheckpointRecord, CheckpointError> {
    let ordering_key = index
        .iter()
        .map(checkpoint_ordering_key)
        .max()
        .ok_or(CheckpointError::CheckpointCorruptionError)?;
    let selected = select_unique_checkpoint_for_ordering_key(index, ordering_key)?;
    validate_selected_checkpoint_record(&selected)?;
    Ok(selected)
}

fn checkpoint_ordering_key(record: &CheckpointRecord) -> CheckpointOrderingKey {
    CheckpointOrderingKey {
        checkpoint_sequence: record.payload.ordering.checkpoint_sequence,
        created_at: record.payload.ordering.created_at,
    }
}

fn select_unique_checkpoint_for_ordering_key(
    index: Vec<CheckpointRecord>,
    ordering_key: CheckpointOrderingKey,
) -> Result<CheckpointRecord, CheckpointError> {
    let mut candidates = index
        .into_iter()
        .filter(|record| checkpoint_ordering_key(record) == ordering_key);
    let selected = candidates
        .next()
        .ok_or(CheckpointError::CheckpointCorruptionError)?;
    if candidates.next().is_some() {
        return Err(CheckpointError::CheckpointCorruptionError);
    }
    Ok(selected)
}

fn validate_selected_checkpoint_record(record: &CheckpointRecord) -> Result<(), CheckpointError> {
    if !record.decodable.0 {
        return Err(CheckpointError::CheckpointCorruptionError);
    }
    if !matches!(record.lifecycle, CheckpointLifecycle::Persisted) {
        return Err(CheckpointError::CheckpointCorruptionError);
    }
    Ok(())
}

/// Build the canonical RPT-1 resume prompt block from base prompt and checkpoint payload.
///
/// Postconditions: output is LF-normalized and list blocks render canonically (`- none` for empty).
pub fn build_resume_prompt_rpt1(
    base_prompt: impl Into<BasePromptText>,
    payload: CheckpointPayload,
) -> Result<ResumePromptText, CheckpointError> {
    let base_prompt = base_prompt.into();
    let normalized_base = normalize_lf(base_prompt.as_ref());
    let objective = normalize_scalar(&payload.objective);
    let context_summary = normalize_scalar(&payload.narrative.context_summary);

    let artifacts = render_list(&payload.narrative.artifacts);
    let decisions = render_list(&payload.narrative.decisions);
    let open_questions = render_list(&payload.narrative.open_questions);

    Ok(ResumePromptText(format!(
        "{normalized_base}

[RPT-1 RESUME CONTEXT]
objective: {objective}
stage_completed: {:?}
next_stage: {:?}
context_summary: {context_summary}
artifacts:
{artifacts}
decisions:
{decisions}
open_questions:
{open_questions}
checkpoint_sequence: {}
created_at: {}",
        payload.stage_completed,
        payload.next_stage,
        payload.ordering.checkpoint_sequence.get(),
        payload.ordering.created_at.to_rfc3339()
    )))
}

/// Execute restart recovery precedence with first-match-wins semantics.
///
/// Preconditions: matrix inputs are canonicalized.
/// Postconditions: first matching branch is selected deterministically and corruption has no fallback.
pub fn execute_restart_recovery_matrix(
    attempt: RecoveryAttempt,
) -> Result<RecoveryOutcome, RecoveryError> {
    if let Some(outcome) = resolve_checkpoint_recovery_branch(attempt.latest_checkpoint) {
        return outcome;
    }
    resolve_transcript_recovery_branch(attempt.transcript_state, attempt.checkpoint_write_state)
}

fn resolve_checkpoint_recovery_branch(
    checkpoint_result: Option<Result<CheckpointRecord, CheckpointError>>,
) -> Option<Result<RecoveryOutcome, RecoveryError>> {
    match checkpoint_result {
        Some(Ok(checkpoint)) => Some(Ok(RecoveryOutcome::ResumeFromCheckpoint(checkpoint))),
        Some(Err(_)) => Some(Err(RecoveryError::CheckpointCorruptionError)),
        None => None,
    }
}

fn resolve_transcript_recovery_branch(
    transcript_state: TranscriptState,
    checkpoint_write_state: CheckpointWriteState,
) -> Result<RecoveryOutcome, RecoveryError> {
    match transcript_state {
        TranscriptState::Decodable => {
            if matches!(
                checkpoint_write_state,
                CheckpointWriteState::PriorWriteError
            ) {
                return Ok(RecoveryOutcome::ResumeFromTranscriptRetryNeeded);
            }
            Ok(RecoveryOutcome::ResumeFromTranscript)
        }
        TranscriptState::Corrupt => Err(RecoveryError::TranscriptCorruptionError),
        TranscriptState::Missing => Err(RecoveryError::MissingSessionStateError),
    }
}

/// Execute restart recovery with session-type guard enforcement.
///
/// Background sessions are excluded from checkpoint/resume flows.
pub fn execute_restart_recovery_for_session(
    request: SessionRecoveryRequest,
) -> Result<RecoveryOutcome, RecoveryError> {
    if matches!(request.session_type, SessionType::Background) {
        return Err(RecoveryError::MissingSessionStateError);
    }
    execute_restart_recovery_matrix(request.attempt)
}

fn canonical_summary_header(segment: &DroppableSegment) -> String {
    format!(
        "[Session summary - turns {} through {}]",
        u32::from(segment.start_turn.get()),
        u32::from(segment.end_turn.get())
    )
}

fn invalid_transition(
    entity: &'static str,
    from: &'static str,
    to: &'static str,
) -> LifecycleError {
    LifecycleError::InvalidTransition { entity, from, to }
}

fn resume_prompt_lifecycle_label(state: ResumePromptLifecycle) -> &'static str {
    match state {
        ResumePromptLifecycle::Draft => "draft",
        ResumePromptLifecycle::Canonicalized => "canonicalized",
        ResumePromptLifecycle::Emitted => "emitted",
    }
}

fn config_snapshot_lifecycle_label(state: ConfigSnapshotLifecycle) -> &'static str {
    match state {
        ConfigSnapshotLifecycle::Loaded => "loaded",
        ConfigSnapshotLifecycle::Validated => "validated",
        ConfigSnapshotLifecycle::Active => "active",
        ConfigSnapshotLifecycle::Rejected => "rejected",
    }
}

fn session_record_lifecycle_label(state: SessionRecordLifecycle) -> &'static str {
    match state {
        SessionRecordLifecycle::Active => "active",
        SessionRecordLifecycle::CompactionRunning => "compaction_running",
        SessionRecordLifecycle::ReadyToSend => "ready_to_send",
        SessionRecordLifecycle::Blocked => "blocked",
    }
}

fn estimate_snapshot_tokens(snapshot: &SessionSnapshot) -> u32 {
    let estimate_tokens = |input: &str| input.split_whitespace().count() as u32;
    let stable_prefix_tokens = estimate_tokens(&snapshot.stable_prefix.bytes);
    let turn_tokens = snapshot
        .turn_pairs
        .iter()
        .map(|turn| {
            estimate_tokens(&turn.user_message.body) + estimate_tokens(&turn.assistant_message.body)
        })
        .sum::<u32>();

    stable_prefix_tokens + turn_tokens
}

fn candidate_score(class: &CandidateClass) -> u32 {
    match class {
        CandidateClass::PureToolExchange => 0,
        CandidateClass::ClearedEmpty => 1,
        CandidateClass::LowSemanticDensity => 2,
    }
}

fn normalize_lf(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_scalar(input: &str) -> String {
    normalize_lf(input)
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_list(items: &[String]) -> String {
    if items.is_empty() {
        return "- none".to_owned();
    }

    items
        .iter()
        .map(|item| format!("- {}", normalize_scalar(item)))
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}
