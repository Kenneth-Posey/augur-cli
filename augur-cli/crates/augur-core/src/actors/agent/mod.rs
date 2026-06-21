//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Agent actor module.
//!
//! The agent actor orchestrates the main conversation loop, managing user turns,
//! LLM interactions, and tool execution. It maintains conversation history and
//! coordinates with other actors (LLM provider, file operations, cache, tools).
//!
//! # Core Types
//!
//! - Agent commands - Sent through `AgentHandle`
//! - Agent services - Dependencies injected at startup
//! - `AgentHandle` - Send-only handle for agent commands

/// Actor loop, commands, and orchestration helpers for the main chat actor.
pub mod agent_actor;
pub(crate) mod agent_actor_ops;
/// Pure helper functions used by the agent actor.
pub mod agent_ops;
/// Extracted deterministic turn-processing core for the agent actor.
mod assistant_core;
/// Public handle for sending agent commands and subscribing to output.
pub mod handle;
/// Owned conversation-history state for the agent actor.
pub mod history;
/// Persistence-related transformations for messages and records.
pub mod persistence_ops;
