//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! History adapter actor module: accepts `Message` items and re-emits typed
//! `HistoryFeedMessage` items to the history feed channel.

pub mod handle;
pub mod history_adapter_actor;
mod history_adapter_actor_ops;
pub mod history_adapter_ops;

pub use handle::HistoryAdapterHandle;
