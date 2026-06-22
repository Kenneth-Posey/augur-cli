//! AgentHandle: the public interface for submitting prompts and receiving output.

use super::agent_actor::AgentCommand;
use super::agent_ops::AgentOutput;
use augur_domain::domain::SdkSessionId;
use augur_domain::domain::string_newtypes::{EndpointName, PromptText, StringNewtype};
use augur_domain::domain::traits::ChatProvider;
use augur_domain::domain::types::{CancelSignal, Message};
use augur_domain::persistence::types::{MessageRecord, MessageType};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};

/// Cloneable handle to a running `AgentActor` task.
///
/// Wraps the command sender, the broadcast output sender, and the cancel watch
/// sender. Non-async submit means callers do not block waiting for the agent;
/// output arrives via the broadcast receiver returned by `subscribe_output`.
/// Multiple callers may hold independent receivers and each will see every output
/// event. `interrupt()` signals the running turn to stop via the watch channel.
#[derive(Clone, bon::Builder)]
pub struct AgentHandle {
    #[builder(setters(vis = "pub(crate)"))]
    tx: mpsc::Sender<AgentCommand>,
    output_tx: broadcast::Sender<AgentOutput>,
    cancel_tx: Arc<watch::Sender<CancelSignal>>,
}

impl AgentHandle {
    /// Create a handle. Called only by `AgentActor::spawn`.
    pub(super) fn new(
        tx: mpsc::Sender<AgentCommand>,
        output_tx: broadcast::Sender<AgentOutput>,
        cancel_tx: Arc<watch::Sender<CancelSignal>>,
    ) -> Self {
        Self::builder()
            .tx(tx)
            .output_tx(output_tx)
            .cancel_tx(cancel_tx)
            .build()
    }

    /// Submit a user prompt for a new conversation turn.
    ///
    /// Non-async: uses `try_send` so the caller never blocks. If the agent
    /// command queue is full or the actor has stopped, the send is silently
    /// dropped. Output arrives asynchronously via the broadcast channel.
    pub fn submit(&self, prompt: PromptText, endpoint: EndpointName) {
        let _ = self.tx.try_send(AgentCommand::Submit { prompt, endpoint });
    }

    /// Subscribe to the agent's output broadcast channel.
    ///
    /// Returns a new `broadcast::Receiver<AgentOutput>`. The TUI actor calls
    /// this at spawn time. Each call creates an independent receiver; no
    /// message is lost to one consumer because another is slow.
    pub fn subscribe_output(&self) -> broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    /// Send a graceful shutdown signal to the agent actor.
    ///
    /// Uses `try_send`; ignores errors if the actor has already stopped.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(AgentCommand::Shutdown);
    }

    /// Restore a previously saved session by replacing conversation history.
    ///
    /// Converts `MessageRecord`s to plain `Message`s, then sends
    /// `AgentCommand::RestoreSession(messages)` via `try_send`. The agent
    /// actor rebuilds `ConversationHistory` from the supplied messages before
    /// the next turn, restoring context across sessions. Error-typed records are
    /// filtered out before sending - they are display annotations only and must
    /// not be sent to the LLM as conversation context. Silently dropped if
    /// the actor command queue is full or the actor has stopped.
    pub fn restore(&self, records: Vec<MessageRecord>) {
        let messages: Vec<Message> = records
            .into_iter()
            .filter(|r| !matches!(r.message_type, MessageType::Error))
            .map(|r| r.message)
            .collect();
        let _ = self.tx.try_send(AgentCommand::RestoreSession(messages));
    }

    /// Signal the currently running turn to stop.
    ///
    /// Sends `true` on the cancel watch channel. The agent actor's `consume_stream`
    /// loop observes this signal via `cancel_rx.changed()` and exits early,
    /// causing `process_turn` to emit `AgentOutput::Interrupted`. Safe to call
    /// when no turn is running - the signal is consumed at the start of the next
    /// `Submit` via `borrow_and_update()`.
    pub fn interrupt(&self) {
        let _ = self.cancel_tx.send(CancelSignal::Cancelled);
    }

    /// Return a snapshot of the current conversation history.
    ///
    /// Sends `AgentCommand::SnapshotHistory` to the actor and awaits the response
    /// on a oneshot channel. Returns an empty `Vec<Message>` if the actor has
    /// stopped or the send fails.
    ///
    /// Used by the TUI to seed the ask panel with the main conversation context
    /// when the ask panel is first opened. The snapshot is frozen at call time;
    /// subsequent turns on the main agent do not affect it.
    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn history_snapshot(&self) -> Vec<augur_domain::domain::types::Message> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .tx
            .try_send(AgentCommand::SnapshotHistory { reply_tx: tx })
            .is_err()
        {
            return Vec::new();
        }
        rx.await.unwrap_or_default()
    }

    /// Read the current cancel signal state. For tests only.
    ///
    /// Returns the semantic cancellation signal value currently held by the
    /// internal watch channel.
    pub(crate) fn is_cancelled(&self) -> CancelSignal {
        *self.cancel_tx.borrow()
    }

    /// Clone the agent's output broadcast sender for forwarding automated replies.
    ///
    /// Returns a clone of the internal `broadcast::Sender<AgentOutput>`. Used by
    /// the wiring layer to route automated LLM message responses into the same
    /// rendering broadcast channel as regular agent responses. Cloning the sender
    /// allows the caller to publish events without subscribing through a receiver.
    pub fn clone_output_tx(&self) -> broadcast::Sender<AgentOutput> {
        self.output_tx.clone()
    }

    /// Get the current agent state (last endpoint and selected model).
    ///
    /// Sends `AgentCommand::GetState` to the actor and awaits the response
    /// on a oneshot channel. Returns `AgentState` with `None` values if the actor
    /// has stopped or the send fails. Safe to call at any time including during
    /// shutdown to persist the current settings.
    pub async fn get_state(&self) -> super::agent_actor::AgentState {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self
            .tx
            .try_send(AgentCommand::GetState { reply_tx: tx })
            .is_err()
        {
            return super::agent_actor::AgentState {
                last_endpoint: None,
                selected_model: None,
            };
        }
        rx.await.unwrap_or(super::agent_actor::AgentState {
            last_endpoint: None,
            selected_model: None,
        })
    }

    /// Wrap this handle as `Arc<dyn ChatProvider>` for use by the TUI actor.
    ///
    /// Called in `wiring.rs` when `copilot_chat.enabled` is false (standard path).
    /// The `Arc` allows the TUI to hold the provider without knowing the concrete type.
    pub fn into_chat_provider(self) -> Arc<dyn ChatProvider> {
        Arc::new(self)
    }
}

impl ChatProvider for AgentHandle {
    /// Submit a user prompt. Forwards `endpoint` when present; uses a safe
    /// placeholder when `None` so callers that don't have an endpoint context
    /// (e.g., tests) still compile.
    fn submit(&self, prompt: PromptText, endpoint: Option<EndpointName>) {
        let ep = endpoint.unwrap_or_else(|| EndpointName::new("default"));
        let _ = self.tx.try_send(AgentCommand::Submit {
            prompt,
            endpoint: ep,
        });
    }

    /// Signal the currently running turn to stop via the cancel watch channel.
    fn interrupt(&self) {
        let _ = self.cancel_tx.send(CancelSignal::Cancelled);
    }

    /// Send a graceful shutdown signal to the agent actor.
    fn shutdown(&self) {
        let _ = self.tx.try_send(AgentCommand::Shutdown);
    }

    /// Restore prior conversation history into the agent.
    fn restore(&self, records: Vec<MessageRecord>) {
        let messages: Vec<Message> = records.into_iter().map(|r| r.message).collect();
        let _ = self.tx.try_send(AgentCommand::RestoreSession(messages));
    }

    /// Subscribe to the agent's output broadcast channel.
    fn subscribe_output(&self) -> broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    /// Set the model to use for subsequent requests.
    fn set_model(&self, model_id: augur_domain::domain::string_newtypes::ModelId) {
        let _ = self.tx.try_send(AgentCommand::SetModel(model_id));
    }

    /// Forward a compact request to the agent actor.
    ///
    /// Sends `AgentCommand::Compact` which causes the agent to apply the
    /// configured message compactor (when set) to the conversation history.
    /// Non-blocking: uses `try_send`; silently drops if the actor channel
    /// is full or stopped.
    fn compact(&self) {
        let _ = self.tx.try_send(AgentCommand::Compact);
    }

    /// Clear the agent's conversation history on session reset.
    ///
    /// When `sdk_session_id` is `None` (fresh session for OpenRouter path),
    /// sends `AgentCommand::ClearHistory` to reset in-memory history so old
    /// messages are not sent to the LLM in subsequent turns. When
    /// `sdk_session_id` is `Some`, this is a no-op because the Copilot SDK
    /// owns its own session context.
    fn replace_session(&self, sdk_session_id: Option<SdkSessionId>) {
        if sdk_session_id.is_none() {
            let _ = self.tx.try_send(AgentCommand::ClearHistory);
        }
    }
}
