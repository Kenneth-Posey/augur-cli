//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Deterministic-orchestrator adapter subtree.
//!
//! Hosts the Phase 3 filesystem, artifact, dispatch, and decision adapters used
//! by the deterministic runtime in later phases.

/// Typed artifact lookup and in-place update boundaries.
pub mod artifact_store;
/// Worker/evaluator background dispatch boundaries.
pub mod background_dispatch;
/// Runtime actor command types.
pub mod commands;
/// Replaceable failure-decision policy boundary.
pub mod decision;
/// Runtime actor implementation.
pub mod deterministic_orchestrator_actor;
/// Public handle for the deterministic runtime actor.
pub mod handle;
/// Local workflow seeding and typed YAML loading.
pub mod loader;
