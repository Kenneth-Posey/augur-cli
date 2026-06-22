//! Command actor entry point: builds the command handle with built-in commands.

use super::command_actor_ops as actor_ops;
use super::handle::CommandHandle;
use augur_domain::tools::definition::ToolDefinition;

/// Build a `CommandHandle` pre-loaded with all built-in slash commands.
///
/// `tools` is the full list of registered tool definitions from the tool
/// registry, passed through to `CommandRegistry` so `/tools` can display them.
/// No tokio task is spawned because the registry is read-only after construction.
///
/// Called once during `wiring::run` after building the tool registry.
pub fn build(tools: &[ToolDefinition]) -> CommandHandle {
    actor_ops::build_handle(tools)
}
