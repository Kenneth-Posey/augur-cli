//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Session persistence subsystem.
//!
//! Provides the data model (`types`), synchronous disk I/O (`store`), and
//! the async `PersistenceHandle` (`handle`) used by the agent actor to
//! auto-save after each completed turn.

pub mod handle;
pub mod plan_persistence;
pub mod store;

pub use augur_domain::persistence::types::*;
pub use handle::PersistenceHandle;
pub use plan_persistence::{PlanPersistenceError, StepArtifactRow};
