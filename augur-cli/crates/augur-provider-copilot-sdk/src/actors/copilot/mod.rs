//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Copilot chat actor: GitHub Copilot SDK session lifecycle and streaming.
//!
//! This module owns a `copilot_sdk::Client + Session`, streams `AgentOutput`
//! events to the TUI via a broadcast channel, and implements `ChatProvider` via
//! `CopilotChatHandle`. `wiring.rs` spawns this actor when
//! `config.copilot_chat.enabled` is true.

pub mod assistant;
pub mod background_agent;
pub mod commands;
pub mod copilot_actor;
pub mod event_classifier;
pub mod handle;

pub mod agent_feed_ops;
pub mod background_event_mapper;
pub mod background_feed_dispatcher;
pub mod event_mapper;
pub mod feed_router;

pub use handle::CopilotChatHandle;
