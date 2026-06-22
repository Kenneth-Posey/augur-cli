//! Command actor: slash-command registry and execution.
//!
//! Processes slash commands entered by the user (e.g., `/clear`, `/save`, `/exit`).
//! Maintains a registry of available commands and routes incoming commands to
//! their respective handlers. Integrates with the agent to execute command actions.

/// Actor entry point: builds the command handle with built-in commands.
pub mod command_actor;
/// Private helper operations for the command actor.
mod command_actor_ops;
/// Public handle to the read-only command registry.
pub mod handle;
/// Pure registry: registering, executing, and listing slash commands.
pub mod registry;
/// Domain types re-exported from `augur_domain::domain::types`.
pub mod types;

pub use handle::CommandHandle;
pub use types::CommandOutcome;
