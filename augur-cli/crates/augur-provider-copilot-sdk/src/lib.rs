//! Copilot-provider workspace crate for guided-plan hook wiring and shared helpers.

/// Actor implementations owned by the Copilot provider crate.
pub mod actors;
/// Guided-plan hook runners owned by the Copilot provider crate.
pub mod guided_plan;
/// Shared Copilot session and permission helpers.
pub mod shared;
