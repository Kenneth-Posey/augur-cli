//! Guided-plan hook runners implemented by the copilot provider crate.

/// Default Copilot-agent hook runner used by guided-plan wiring.
pub mod copilot_agent;

/// Re-export the provider-owned Copilot hook runner builder.
pub use copilot_agent::build_copilot_hook_runner;
