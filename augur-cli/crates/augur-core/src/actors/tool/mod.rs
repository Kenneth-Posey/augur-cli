//! Tool actor module.
//!
//! Hosts the leaf actor that receives tool calls, dispatches them through the
//! tool registry, and returns structured results.

/// Public handle and executor trait re-export for tool dispatch.
pub mod handle;
/// Inline executor that runs tools directly against a registry without an actor.
pub mod inline_executor;
/// Actor task that executes registered tools.
pub mod tool_actor;
/// Private helper operations delegated from `actor`.
mod tool_actor_ops;
/// Command and helper types for tool execution.
pub mod tool_ops;

pub use inline_executor::InlineToolExecutor;
