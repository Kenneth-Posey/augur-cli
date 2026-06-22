//! Shared request type for the structured query-user tool.

use crate::domain::string_newtypes::{ChoiceText, OutputText, PromptText, ToolName};
use tokio::sync::{mpsc, oneshot};

/// A pending question from the LLM waiting for a human answer.
///
/// Created by the query-user tool. The TUI actor receives this over the mpsc
/// channel, enters query mode, and sends the user's resolved answer back
/// through `reply_tx`. The agent turn is suspended until the reply arrives.
#[derive(bon::Builder)]
pub struct QueryUserRequest {
    /// The question text displayed in the query overlay.
    pub question: PromptText,
    /// Optional choices for the user to select with up/down arrows. May be empty.
    pub choices: Vec<ChoiceText>,
    /// Oneshot sender; the TUI sends the resolved answer back on this channel.
    pub reply_tx: oneshot::Sender<OutputText>,
}

/// Tool that lets the LLM pause its turn and ask the user a structured question.
///
/// This type is intentionally shared so provider crates can build requests
/// without depending on the core implementation module.
pub struct QueryUserTool {
    request_tx: mpsc::Sender<QueryUserRequest>,
}

impl QueryUserTool {
    pub fn new(request_tx: mpsc::Sender<QueryUserRequest>) -> Self {
        Self { request_tx }
    }
}
