//! Private helper operations for the command actor.

use super::handle::CommandHandle;
use super::registry::CommandRegistry;
use augur_domain::tools::definition::ToolDefinition;

/// Build a command handle backed by the built-in command registry.
pub(super) fn build_handle(tools: &[ToolDefinition]) -> CommandHandle {
    CommandHandle::new(CommandRegistry::with_builtins(tools))
}
