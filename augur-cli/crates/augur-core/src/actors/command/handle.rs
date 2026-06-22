//! CommandHandle: public interface to the command registry.

use super::registry::CommandRegistry;
use super::types::{CommandDef, CommandOutcome};
use augur_domain::domain::StringNewtype;
use augur_domain::domain::string_newtypes::PromptText;
use std::sync::Arc;

/// Cheaply cloneable handle to the read-only command registry.
///
/// Wraps an `Arc<CommandRegistry>` so it can be stored in `TuiSpawnArgs` and
/// cloned into any context that needs to execute commands or produce completions.
/// No task is required: the registry is read-only after construction.
#[derive(Clone)]
pub struct CommandHandle(Arc<CommandRegistry>);

impl CommandHandle {
    /// Create a handle wrapping the given registry. Called only by `actor::build`.
    pub(super) fn new(registry: CommandRegistry) -> Self {
        CommandHandle(Arc::new(registry))
    }

    /// Execute a prompt string and return the appropriate outcome.
    ///
    /// Delegates to `CommandRegistry::execute`. The TUI actor matches on the
    /// returned `CommandOutcome` to decide whether to quit, switch endpoint,
    /// display a system message, or submit to the agent.
    pub fn execute(&self, text: &PromptText) -> CommandOutcome {
        self.0.execute(text)
    }

    /// Return matching `CommandDef` completions for the current prompt buffer.
    ///
    /// `buffer` is the raw prompt text (including the leading `/`). The method
    /// strips the `/` prefix before delegating to the registry. Returns an empty
    /// vec when `buffer` does not start with `/`. Results are alpha-sorted and
    /// capped at `MAX_COMPLETIONS` by the registry.
    pub fn completions_for(&self, buffer: &PromptText) -> Vec<CommandDef> {
        let prefix = match buffer.as_str().strip_prefix('/') {
            Some(p) => p,
            None => return vec![],
        };
        self.0.completions(&PromptText::from(prefix))
    }

    /// Return all registered command definitions.
    ///
    /// Used when the full command list is needed independent of any typed prefix,
    /// e.g. for generating documentation or displaying a static help panel.
    pub fn all_commands(&self) -> &[CommandDef] {
        self.0.all_commands()
    }
}
