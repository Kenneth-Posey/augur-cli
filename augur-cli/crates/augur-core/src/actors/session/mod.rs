//! Session actor module.
//!
//! Owns the active endpoint state, publishes snapshots over a watch channel, and
//! processes endpoint-change commands over an mpsc channel.

/// Public handle for reading snapshots and sending commands.
pub mod handle;
/// Actor task that owns endpoint state and processes commands.
pub mod session_actor;
/// Private helper operations for the session actor.
mod session_actor_ops;
/// Command types processed by the session actor.
pub mod session_ops;
