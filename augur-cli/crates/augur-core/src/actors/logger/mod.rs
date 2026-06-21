//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Logger actor module: records all LLM conversation messages to JSONL files.

pub mod handle;
pub mod logger_actor;
mod logger_actor_ops;
pub mod logger_ops;

pub use handle::LoggerHandle;
