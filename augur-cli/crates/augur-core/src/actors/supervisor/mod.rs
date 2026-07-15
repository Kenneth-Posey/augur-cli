//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Supervisor actor module.
//!
//! The supervisor actor monitors and manages the entire agent actor system,
//! handling shutdown coordination, error recovery, and inter-actor messaging.
//! It acts as the system's central orchestrator for actor lifecycle management.

/// Supervisor command types.
pub mod commands;
/// Public handle for supervisor commands and event subscription.
pub mod handle;
/// Supervisor actor loop and event-channel lifecycle.
pub mod supervisor_actor;

pub use handle::SupervisorHandle;
pub use supervisor_actor::SupervisorActor;
