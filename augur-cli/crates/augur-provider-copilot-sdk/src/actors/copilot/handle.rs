//! `CopilotChatHandle`: the public interface to the `CopilotChatActor`.

use super::commands::CopilotChatCmd;
use augur_domain::channels::AGENT_OUTPUT_CAPACITY;
use augur_domain::persistence::types::MessageRecord;
use augur_domain::string_newtypes::{
    AgentName, EndpointName, FilePath, ModelId, PromptText, SdkSessionId,
};
use augur_domain::traits::ChatProvider;
use augur_domain::types::AgentOutput;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

/// Cloneable handle to a running `CopilotChatActor`.
///
/// Wraps the command mpsc sender and the output broadcast sender. Non-async
/// submit means callers do not block. Multiple callers may hold independent
/// receivers via `subscribe_output`; each sees every output event.
/// Implements `ChatProvider` so the TUI actor can use it interchangeably with
/// `AgentHandle` via `Arc<dyn ChatProvider>`.
#[derive(Clone)]
pub struct CopilotChatHandle {
    cmd_tx: mpsc::Sender<CopilotChatCmd>,
    output_tx: broadcast::Sender<AgentOutput>,
}

impl CopilotChatHandle {
    /// Construct a handle. Called only by `CopilotChatActor::spawn`.
    pub(super) fn new(
        cmd_tx: mpsc::Sender<CopilotChatCmd>,
        output_tx: broadcast::Sender<AgentOutput>,
    ) -> Self {
        CopilotChatHandle { cmd_tx, output_tx }
    }
}

impl ChatProvider for CopilotChatHandle {
    /// Submit a user message to the Copilot session.
    ///
    /// Ignores `endpoint` - the Copilot SDK selects the model from its own session
    /// config. Non-blocking: uses `try_send`; silently drops on full channel.
    /// Sends `attachments: []` via `SendMessage { text, attachments: vec![] }`.
    fn submit(&self, prompt: PromptText, _endpoint: Option<EndpointName>) {
        let _ = self.cmd_tx.try_send(CopilotChatCmd::SendMessage {
            text: prompt,
            attachments: vec![],
        });
    }

    /// No-op for Copilot: SDK does not support mid-turn cancellation at this time.
    fn interrupt(&self) {}

    /// Send a graceful shutdown signal to the actor.
    fn shutdown(&self) {
        let _ = self.cmd_tx.try_send(CopilotChatCmd::Shutdown);
    }

    /// No-op: Copilot SDK owns session context; external history injection is unsupported.
    fn restore(&self, records: Vec<MessageRecord>) {
        let _ = self.cmd_tx.try_send(CopilotChatCmd::Restore(records));
    }

    /// Subscribe to the Copilot actor's output broadcast channel.
    fn subscribe_output(&self) -> broadcast::Receiver<AgentOutput> {
        self.output_tx.subscribe()
    }

    /// Forward a compact request to the Copilot actor.
    ///
    /// Sends `CopilotChatCmd::Compact` which causes the actor to call
    /// `session.compact()` on the active GitHub Copilot SDK session,
    /// compressing the conversation context window. Non-blocking: uses
    /// `try_send`; silently drops if the actor channel is full or stopped.
    fn compact(&self) {
        let _ = self.cmd_tx.try_send(CopilotChatCmd::Compact);
    }

    /// Send a `RunBackgroundAgent` command to the Copilot actor.
    ///
    /// Non-blocking: uses `try_send`; silently drops if the actor channel is
    /// full or stopped.
    fn run_background_agent(&self, agent: AgentName, prompt: PromptText) {
        let _ = self
            .cmd_tx
            .try_send(CopilotChatCmd::RunBackgroundAgent { agent, prompt });
    }

    /// Submit a user prompt with file attachments to the Copilot session.
    ///
    /// Overrides the `ChatProvider` default to pass `attachments` through the
    /// Copilot SDK `MessageOptions::attachments` field. Each `FilePath` is
    /// converted to a `UserMessageAttachment` by `session_ops::build_sdk_attachments`
    /// inside the actor. Non-blocking: uses `try_send`.
    fn submit_with_attachments(
        &self,
        prompt: PromptText,
        _endpoint: Option<EndpointName>,
        attachments: Vec<FilePath>,
    ) {
        let _ = self.cmd_tx.try_send(CopilotChatCmd::SendMessage {
            text: prompt,
            attachments,
        });
    }

    /// Switch the active model by sending `SetModel` to the Copilot actor.
    ///
    /// Overrides the `ChatProvider` default. Non-blocking: uses `try_send`;
    /// silently drops if the actor channel is full or stopped.
    fn set_model(&self, model_id: ModelId) {
        let _ = self.cmd_tx.try_send(CopilotChatCmd::SetModel {
            model_id,
            reasoning_effort: None,
        });
    }

    /// Switch the active model with an explicit reasoning effort level.
    ///
    /// Overrides the `ChatProvider` default. Sends `SetModel` with both the
    /// model id and the selected reasoning effort level to the Copilot actor.
    /// Non-blocking: uses `try_send`.
    fn set_model_with_options(
        &self,
        model_id: ModelId,
        reasoning_effort: Option<augur_domain::thinking_mode::ReasoningEffort>,
    ) {
        let _ = self.cmd_tx.try_send(CopilotChatCmd::SetModel {
            model_id,
            reasoning_effort,
        });
    }

    /// Replace the active SDK session by sending `ReplaceSession` to the actor.
    ///
    /// Overrides the `ChatProvider` default. When `sdk_session_id` is `Some(id)`,
    /// the actor resumes the specified SDK session. When `None`, the actor creates
    /// a fresh session with no prior context. Non-blocking: uses `try_send`.
    fn replace_session(&self, sdk_session_id: Option<SdkSessionId>) {
        let _ = self
            .cmd_tx
            .try_send(CopilotChatCmd::ReplaceSession { sdk_session_id });
    }
}

/// Create the output broadcast channel for the Copilot chat actor.
///
/// Uses `AGENT_OUTPUT_CAPACITY` to match the agent actor's output channel size.
/// Called once in `CopilotChatActor::spawn`; the sender is stored in the handle
/// and cloned into the actor task. The initial receiver is dropped; all consumers
/// call `subscribe_output()` on the handle.
pub(super) fn make_output_channel() -> broadcast::Sender<AgentOutput> {
    let (tx, _) = broadcast::channel(*AGENT_OUTPUT_CAPACITY);
    tx
}

/// Wrap a `CopilotChatHandle` as `Arc<dyn ChatProvider>`.
///
/// Convenience function for `wiring.rs` so the Copilot path can hand the TUI
/// a type-erased provider in a single call.
pub fn into_chat_provider(handle: CopilotChatHandle) -> Arc<dyn ChatProvider> {
    Arc::new(handle)
}
