//! Inbound command types for `CopilotChatActor`.

use augur_domain::persistence::types::MessageRecord;
use augur_domain::string_newtypes::{AgentName, FilePath, ModelId, PromptText, SdkSessionId};

/// Commands sent to the `CopilotChatActor` through its mpsc command channel.
///
/// `SendMessage` drives a new conversation turn through the Copilot SDK session.
/// `Restore` seeds `LogState.message_history` so subsequent `save_turn` calls
/// include the full prior conversation. The Copilot SDK owns session context and
/// does not accept injected history, so the SDK session itself is unchanged.
/// `Compact` requests the session to compress its context window.
/// `ReplaceSession` closes the current SDK session and opens a new or resumed one.
/// `Shutdown` cleanly stops the actor and the underlying CLI subprocess.
pub enum CopilotChatCmd {
    /// Send a user message to the active Copilot session.
    ///
    /// `text` is the prompt string. `attachments` is the list of file paths
    /// parsed from `@token` syntax; an empty vec sends `attachments: []` in
    /// the SDK payload which is required to avoid null-attachment errors.
    SendMessage {
        text: PromptText,
        attachments: Vec<FilePath>,
    },
    /// Seed the log state history from a restored session record.
    ///
    /// The Copilot SDK manages session context internally; the SDK session is
    /// not affected. The supplied records are stored in `LogState.message_history`
    /// so that subsequent `save_turn` calls correctly append to the full history
    /// rather than writing only the current turn.
    Restore(Vec<MessageRecord>),
    /// Compact the session's conversation context window.
    Compact,
    /// Switch the active model for the running session.
    ///
    /// Calls `session.set_model(model_id, None)` on the underlying SDK session.
    /// After a successful switch, the actor emits `AgentOutput::ActiveModelChanged`
    /// with the new model id so the TUI status bar updates immediately.
    SetModel {
        /// The model id to switch to (empty string means "auto").
        model_id: ModelId,
        /// Optional reasoning effort level passed to `SetModelOptions`.
        /// `None` means default (no extended thinking override).
        reasoning_effort: Option<augur_domain::thinking_mode::ReasoningEffort>,
    },
    /// Close the current SDK session and open a new or resumed one.
    ///
    /// When `sdk_session_id` is `Some(id)`, the actor calls `resume_session` to
    /// reconnect to the specified SDK session. When `None`, the actor calls
    /// `create_session` to start a fresh SDK session with no prior context.
    /// Used by: session picker restore (to reconnect to saved SDK context) and
    /// the `/new-session` command (to start a fresh session).
    ReplaceSession {
        sdk_session_id: Option<SdkSessionId>,
    },
    /// Launch a background SDK agent session and stream feed events.
    RunBackgroundAgent {
        agent: AgentName,
        prompt: PromptText,
    },
    /// Gracefully stop the actor and disconnect the CLI subprocess.
    Shutdown,
}
