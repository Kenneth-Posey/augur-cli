//! Command types for the TUI agent panel actor.

use augur_domain::domain::types::AgentFeedOutput;

/// Commands accepted by the TUI agent panel actor.
///
/// `AgentFeed` and `ToolFeed` both carry an `AgentFeedOutput` item and are
/// forwarded to the unified output channel. `Shutdown` stops the actor loop.
#[derive(Debug)]
pub enum AgentPanelCmd {
    /// An item from a background agent message feed.
    AgentFeed(AgentFeedOutput),
    /// An item from a background tool message feed.
    ToolFeed(AgentFeedOutput),
    /// Graceful shutdown: the actor exits its run loop.
    Shutdown,
}
