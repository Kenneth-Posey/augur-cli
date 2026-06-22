//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Executor actor - CLI session driver bridging `copilot-sdk-rust` to domain types.
//!
//! Manages Copilot CLI session execution through the Copilot SDK, translating
//! agent output into CLI commands and streaming responses back to the agent.
//! Handles session lifecycle, error recovery, and output streaming.

pub mod commands;
pub mod event_mapper;
pub mod executor_actor;
pub mod executor_ops;
pub mod handle;

pub use handle::ExecutorHandle;
