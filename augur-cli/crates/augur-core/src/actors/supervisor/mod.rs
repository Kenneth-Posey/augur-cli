//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Supervisor actor module.
//!
//! The supervisor actor monitors and manages the entire agent actor system,
//! handling shutdown coordination, error recovery, and inter-actor messaging.
//! It acts as the system's central orchestrator for actor lifecycle management.

/// Checkpoint heuristics for plan execution.
pub mod checkpoint;
/// Supervisor command types.
pub mod commands;
/// Public handle for supervisor commands and event subscription.
pub mod handle;
/// Meta-planning prompt construction and helpers.
pub mod meta_planner;
/// Pure gate evaluation for executor step outcomes.
pub mod phase_gate;
/// Supervisor actor loop and execution orchestration.
pub mod supervisor_actor;

pub use handle::SupervisorHandle;
pub use supervisor_actor::SupervisorActor;
