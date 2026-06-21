//! LSP actor module: drives a rust-analyzer child process and exposes a
//! channel-backed handle for tool and wiring code.
//!
//! # Public surface
//!
//! **`LspHandle`** - the primary public type; a cloneable channel-backed
//! reference to the running `LspActor`. This is the type consumed by tools
//! and wiring code.
//!
//! **`actor`** - exposed as `pub(crate) mod` so that wiring code can call
//! `actor::spawn` and supply `actor::LspActorConfig`
//! (see IC-08 in the dependency graph). All types inside `actor` that must not
//! escape the module are `pub(super)`: `LspActorState`, `LspPhase`, `JsonRpcMsg`.
//!
//! **`LspRequest`** remains `pub(crate)` for internal actor tests.
//!
//! # Module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `handle.rs` | `LspHandle` (channel handle) and `LspRequest` (channel message) |
//! | `actor.rs`  | `spawn`, `LspActorConfig`, private run-loop state |
//! | `actor_ops.rs` | Nine private helper functions called only from `actor.rs` |
//!
//! See `IC-01` in `plans/lsp-query-tool/plan/dependency-graph.md` for the
//! interface contract between this module and `domain::lsp`.

/// Channel handle and message types.
mod handle;

/// Actor spawn factory, run loop, and private state types.
pub mod lsp_actor;

/// Private helper operations for the actor run loop.
/// Not accessible outside `actors::lsp`.
pub mod lsp_actor_ops;

/// The only public surface of the `actors::lsp` module.
///
/// Cloneable channel-backed reference to the running `LspActor`.
/// All tools and wiring code import only this type from this module.
/// See [`handle::LspHandle`] for the full documentation.
pub use handle::LspHandle;

#[allow(unused_imports)]
pub(crate) use handle::LspRequest;
