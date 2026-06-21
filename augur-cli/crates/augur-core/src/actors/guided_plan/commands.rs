//! Commands sent to the `GuidedPlanActor` via its mpsc channel.

use augur_domain::domain::guided_plan::GuidedPlanConfig;
use augur_domain::domain::FilePath;

/// Commands accepted by the `GuidedPlanActor`.
///
/// Consumers: `GuidedPlanHandle` methods (the sole producers);
/// `actor::run_loop` (the sole consumer).
#[derive(Debug)]
pub enum GuidedPlanCmd {
    /// Load a plan and transition phase 0 to `InProgress`.
    ///
    /// Sender: `/run-plan` command handler in `key_dispatch::handle_submit`.
    /// Precondition: no plan is currently running (any existing state is replaced).
    Start {
        /// Parsed plan configuration from the YAML frontmatter.
        config: GuidedPlanConfig,
        /// Path to the plan file, used for display and diagnostic messages.
        plan_path: FilePath,
    },
    /// Confirm that the current phase work is complete; begins hook execution.
    ///
    /// Sender: TUI key handler (`Enter` in `ConversationMode::GuidedPlan`).
    /// Precondition: current phase is `InProgress`.
    ConfirmPhase,
    /// Override a `NeedsRework` gate and advance unconditionally.
    ///
    /// Sender: TUI key handler (`F10` in `ConversationMode::GuidedPlan`).
    /// Precondition: current phase is `NeedsRework`. This is a destructive
    /// override; the actor logs a warning before advancing.
    ForceAdvance,
    /// Notify the actor that conversation compaction has completed.
    ///
    /// Sender: TUI actor when a `CompactionComplete` signal is received after
    /// the guided plan actor emitted `GuidedPlanEvent::CompactRequested`.
    CompactionDone,
    /// Shut down the actor loop.
    ///
    /// Sender: `wiring::run` during shutdown.
    Shutdown,
}
