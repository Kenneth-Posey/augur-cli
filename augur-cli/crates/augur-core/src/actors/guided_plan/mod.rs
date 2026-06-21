//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Guided plan execution actor: file-driven, phase-gated plan runner.
//!
//! Provides deterministic plan execution from YAML-frontmattered plan files.
//! Exposes the actor handle, domain event type, and file loader.

/// Command types sent to the guided-plan actor.
pub mod commands;
/// Actor loop and state-machine orchestration for guided plans.
pub mod guided_plan_actor;
/// Public handle for guided-plan commands and events.
pub mod handle;
/// Hook runners used by guided-plan post-phase execution.
pub mod hooks;
/// YAML plan-file loader for guided-plan execution.
pub mod loader;

pub use augur_domain::domain::guided_plan::GuidedPlanEvent;
pub use guided_plan_actor::spawn;
pub use handle::GuidedPlanHandle;
pub use loader::load_guided_plan;
