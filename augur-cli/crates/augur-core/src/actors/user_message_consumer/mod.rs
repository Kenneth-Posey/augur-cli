//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! User message consumer actor module: accepts raw user input and routes to typed output channels.

pub mod handle;
pub mod user_message_consumer_actor;
mod user_message_consumer_actor_ops;
pub mod user_message_consumer_ops;

pub use handle::UserMessageConsumerHandle;
