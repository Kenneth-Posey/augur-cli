//! Ask-panel handle: cloneable interface to the ask-panel agent actor.

use crate::actors::agent::agent_ops::AgentOutput;
use crate::actors::agent::handle::AgentHandle;
use augur_domain::domain::string_newtypes::{EndpointName, PromptText};
use augur_domain::domain::types::Message;
use augur_domain::persistence::types::MessageRecord;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;

/// Cloneable handle to the running ask-panel actor.
///
/// Wraps an `AgentHandle` configured with a read-only tool registry and
/// delegates all commands to it. The internal `AgentHandle` is Clone; the
/// `tool_join` Arc is shared across clones so wiring can take it once via
/// `take_tool_join` for clean shutdown. `default_endpoint` is the fixed LLM
/// endpoint this panel submits to, always a real config endpoint (never Copilot).
///
/// Ownership: constructed by `actor::spawn`. Consumed by wiring and the TUI.
#[derive(Clone, bon::Builder)]
pub struct AskHandle {
    inner: AgentHandle,
    tool_join: Arc<Mutex<Option<JoinHandle<()>>>>,
    default_endpoint: EndpointName,
}

impl AskHandle {
    /// Take the ToolActor JoinHandle for wiring shutdown.
    ///
    /// Returns `Some` on the first call; subsequent calls return `None`.
    /// Consumers: `wiring.rs` awaits this handle during shutdown sequencing.
    #[tracing::instrument(skip(self))]
    pub async fn take_tool_join(&self) -> Option<JoinHandle<()>> {
        self.tool_join.lock().await.take()
    }

    /// Submit a user prompt to the ask-panel agent.
    ///
    /// Non-async: uses `try_send` so the caller never blocks. Output arrives
    /// asynchronously via `subscribe_output`. Always uses the stored
    /// `default_endpoint` - do not pass `state.agent.endpoint_name` here
    /// because that may point to a Copilot endpoint the standard LLM actor
    /// cannot resolve.
    pub fn submit(&self, prompt: PromptText) {
        self.inner.submit(prompt, self.default_endpoint.clone());
    }

    /// Return the default LLM endpoint this ask panel submits to.
    ///
    /// Always a real endpoint from `config.endpoints`. Never a Copilot endpoint.
    /// Consumers: `key_dispatch::handle_ask_submit`.
    pub fn default_endpoint(&self) -> &EndpointName {
        &self.default_endpoint
    }

    /// Signal the currently running ask turn to stop.
    ///
    /// Delegates to `AgentHandle::interrupt` via the cancel watch channel.
    pub fn interrupt(&self) {
        self.inner.interrupt();
    }

    /// Send a graceful shutdown signal to the ask-panel agent.
    ///
    /// Delegates to `AgentHandle::shutdown` via `try_send`.
    pub fn shutdown(&self) {
        self.inner.shutdown();
    }

    /// Subscribe to the ask-panel agent's output broadcast channel.
    ///
    /// Returns a new `broadcast::Receiver<AgentOutput>`. Each call creates an
    /// independent receiver; messages are not lost because one consumer is slow.
    pub fn subscribe_output(&self) -> broadcast::Receiver<AgentOutput> {
        self.inner.subscribe_output()
    }

    /// Restore a previously saved ask-panel session history.
    ///
    /// Converts `MessageRecord`s and delegates to `AgentHandle::restore`.
    /// Used by Phase 6 persistence on session resume.
    pub fn restore(&self, records: Vec<MessageRecord>) {
        self.inner.restore(records);
    }

    /// Return a snapshot of the ask-panel conversation history.
    ///
    /// Async: sends a oneshot request to the agent actor. Returns empty vec
    /// if the actor has stopped. Used by Phase 6 persistence for disk save.
    #[tracing::instrument(skip(self))]
    pub async fn history_snapshot(&self) -> Vec<Message> {
        self.inner.history_snapshot().await
    }
}
