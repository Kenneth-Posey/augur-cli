//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! LLM feed consumer actor module: classifies and routes `StreamChunk` items to typed output channels.

pub mod handle;
pub mod llm_feed_consumer_actor;
mod llm_feed_consumer_actor_ops;
pub mod llm_feed_consumer_ops;

pub use handle::LlmFeedConsumerHandle;
