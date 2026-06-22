//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Cache actor: dependency-graph-driven Anthropic prompt cache management.
//!
//! Manages Anthropic's prompt caching feature to reduce LLM inference costs.
//! Maintains a tiered snapshot of recently read files and project state,
//! enabling efficient cache hits across multiple agent turns.
//!
//! # Key Concepts
//!
//! - **Cache Tiers** - Files grouped by importance (active, session, project)
//! - **Snapshots** - Immutable file states captured at specific points
//! - **Dependencies** - Codebase dependency graph for smart cache invalidation

pub mod cache_actor;
mod cache_actor_ops;
pub mod cache_ops;
pub mod deps;
pub mod handle;
pub mod tiers;
