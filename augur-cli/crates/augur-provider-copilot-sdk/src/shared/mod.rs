//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Shared helpers reused across multiple actor modules.
//!
//! Provides common utilities for actor communication, permission checking,
//! error handling, and cross-actor coordination. Used by all actor modules
//! to ensure consistency in behavior and error reporting.

/// Copilot SDK permission helpers shared by actor-layer integrations.
pub mod copilot_permissions;
/// Copilot SDK session isolation helpers to avoid cross-app session contamination.
pub mod copilot_session_identity;
