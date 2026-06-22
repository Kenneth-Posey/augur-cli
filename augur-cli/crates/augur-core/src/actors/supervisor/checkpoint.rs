//! Checkpoint heuristic tracking for the supervisor actor.
//!
//! `CheckpointTracker` accumulates per-step file-change counts,
//! then reports whether a checkpoint should fire. When
//! `should_trigger` returns `true`, the supervisor fires the checkpoint
//! actions and calls `reset`.

use augur_domain::domain::plan_tree::CheckpointConfig;
use augur_domain::domain::{Count, NumericNewtype};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Number of file changes that triggers an automatic checkpoint.
///
/// Each step that produces a `PlanNodeUpdate::Done` increments the counter.
/// When it reaches this threshold a checkpoint fires even if the plan node
/// carries no `CheckpointConfig`.
pub const CHECKPOINT_FILE_THRESHOLD: Count = Count::of(10);

// ── CheckpointTracker ─────────────────────────────────────────────────────────

/// Accumulates per-step heuristics and decides when a checkpoint should fire.
///
/// The supervisor holds one instance in `SupervisorState`. After each step
/// completes, it calls `record_file_change`, then checks
/// `should_trigger(node.checkpoint_config.as_ref())`. If triggered it
/// fires the checkpoint actions and calls `reset`.
#[derive(Debug, Default)]
pub struct CheckpointTracker {
    /// Number of completed file-changing steps since the last reset.
    file_delta: Count,
}

/// Semantic checkpoint decision emitted by `CheckpointTracker::should_trigger`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointTriggerDecision(bool);

impl From<CheckpointTriggerDecision> for bool {
    fn from(value: CheckpointTriggerDecision) -> Self {
        value.0
    }
}

impl CheckpointTracker {
    /// Increments the file-change counter by one.
    ///
    /// Call after every step where a `PlanNodeUpdate::Done` is observed.
    pub fn record_file_change(&mut self) {
        self.file_delta += Count::of(1);
    }

    /// Returns `true` if a checkpoint should fire now.
    ///
    /// Checkpoint fires when any of the following conditions holds:
    /// 1. `config` is `Some` and `config.commit` is `true` (explicit marker).
    /// 2. `config` is `Some` and `config.compact` is `true` (compact-only trigger).
    /// 3. `file_delta >= CHECKPOINT_FILE_THRESHOLD`.
    pub(crate) fn should_trigger(
        &self,
        config: Option<&CheckpointConfig>,
    ) -> CheckpointTriggerDecision {
        let explicit = config.map(|c| c.commit.0 || c.compact.0).unwrap_or(false);
        let file_heuristic = self.file_delta >= CHECKPOINT_FILE_THRESHOLD;
        CheckpointTriggerDecision(explicit || file_heuristic)
    }

    /// Resets the file counter to zero after a checkpoint fires.
    pub fn reset(&mut self) {
        self.file_delta = Count::ZERO;
    }
}
