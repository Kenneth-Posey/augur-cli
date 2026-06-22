//! Shared actor-facing handle and command contracts.

use crate::domain::feeds::HistoryFeedMessage;
use crate::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use crate::domain::types::{ContextUsageStats, LlmUsage, Message, ProjectTokenTotals};
use tokio::sync::{mpsc, oneshot};

/// Commands processed by the token-tracker actor task.
#[derive(Debug)]
pub enum TokenTrackerCommand {
    RecordUsage(LlmUsage),
    RecordContext(ContextUsageStats),
    ResetTotals,
    Snapshot(oneshot::Sender<ProjectTokenTotals>),
    ContextSnapshot(oneshot::Sender<Option<ContextUsageStats>>),
    Shutdown,
}

/// Cloneable handle to the running token-tracker actor.
#[derive(Clone)]
pub struct TokenTrackerHandle {
    tx: mpsc::Sender<TokenTrackerCommand>,
}

impl TokenTrackerHandle {
    pub fn new(tx: mpsc::Sender<TokenTrackerCommand>) -> Self {
        Self { tx }
    }

    pub fn record_usage(&self, usage: LlmUsage) {
        let _ = self.tx.try_send(TokenTrackerCommand::RecordUsage(usage));
    }

    pub fn record_context(&self, stats: ContextUsageStats) {
        let _ = self.tx.try_send(TokenTrackerCommand::RecordContext(stats));
    }

    pub fn reset_totals(&self) {
        let _ = self.tx.try_send(TokenTrackerCommand::ResetTotals);
    }

    pub async fn snapshot(&self) -> ProjectTokenTotals {
        let (tx, rx) = oneshot::channel();
        if self
            .tx
            .send(TokenTrackerCommand::Snapshot(tx))
            .await
            .is_err()
        {
            return ProjectTokenTotals::default();
        }
        rx.await.unwrap_or_default()
    }

    pub async fn context_snapshot(&self) -> Option<ContextUsageStats> {
        let (tx, rx) = oneshot::channel();
        if self
            .tx
            .send(TokenTrackerCommand::ContextSnapshot(tx))
            .await
            .is_err()
        {
            return None;
        }
        rx.await.unwrap_or(None)
    }

    pub fn shutdown(&self) {
        let _ = self.tx.try_send(TokenTrackerCommand::Shutdown);
    }
}

impl std::fmt::Debug for TokenTrackerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenTrackerHandle").finish_non_exhaustive()
    }
}

/// Commands sent to the logger actor task.
#[derive(Debug)]
pub enum LogCommand {
    LogMessages {
        endpoint: EndpointName,
        messages: Vec<Message>,
    },
    LogLine {
        role: String,
        content: String,
    },
    LogHistoryEntry(HistoryFeedMessage),
    /// Write one LLM raw request/response/tool-call line to the JSONL log.
    LogLlmRaw {
        /// Flow direction: "request", "response", or "tool_call".
        direction: String,
        /// Provider name, e.g. "openai", "anthropic", "openrouter".
        provider: String,
        /// Model identifier at the time of the request.
        model: String,
        /// Full JSON body for request/tool_call, or token summary for response.
        body: String,
    },
    Shutdown,
}

/// Fire-and-forget handle to the running logger actor.
#[derive(Clone)]
pub struct LoggerHandle {
    tx: mpsc::Sender<LogCommand>,
}

impl LoggerHandle {
    pub fn new(tx: mpsc::Sender<LogCommand>) -> Self {
        Self { tx }
    }

    #[tracing::instrument(skip(self))]
    pub async fn log_messages(&self, endpoint: EndpointName, messages: Vec<Message>) {
        let _ = self
            .tx
            .send(LogCommand::LogMessages { endpoint, messages })
            .await;
    }

    pub fn shutdown(&self) {
        let _ = self.tx.try_send(LogCommand::Shutdown);
    }

    pub fn log_line(&self, role: OutputText, content: OutputText) {
        let _ = self.tx.try_send(LogCommand::LogLine {
            role: role.into_inner(),
            content: content.into_inner(),
        });
    }

    pub fn log_history_entry(&self, entry: HistoryFeedMessage) {
        let _ = self.tx.try_send(LogCommand::LogHistoryEntry(entry));
    }

    /// Write one LLM raw request/response/tool-call line to the JSONL message log.
    ///
    /// Inputs: `direction` – "request", "response", or "tool_call"; `provider` – provider
    /// name string; `model` – model identifier; `body` – full JSON body or summary string.
    /// Fire-and-forget via `try_send`; safe to call from synchronous context.
    pub fn log_llm_raw(&self, direction: &str, provider: &str, model: &str, body: String) {
        let _ = self.tx.try_send(LogCommand::LogLlmRaw {
            direction: direction.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            body,
        });
    }
}

impl std::fmt::Debug for LoggerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggerHandle").finish_non_exhaustive()
    }
}

/// Commands accepted by the history adapter actor.
#[derive(Debug)]
pub enum HistoryAdapterCmd {
    RecordUser(Message),
    RecordLlm(Message),
    Shutdown,
}

/// Fire-and-forget handle to the running history adapter actor.
#[derive(Clone)]
pub struct HistoryAdapterHandle {
    tx: mpsc::Sender<HistoryAdapterCmd>,
}

impl HistoryAdapterHandle {
    pub fn new(tx: mpsc::Sender<HistoryAdapterCmd>) -> Self {
        Self { tx }
    }

    pub fn record_user(&self, msg: Message) {
        let _ = self.tx.try_send(HistoryAdapterCmd::RecordUser(msg));
    }

    pub fn record_llm(&self, msg: Message) {
        let _ = self.tx.try_send(HistoryAdapterCmd::RecordLlm(msg));
    }

    pub fn shutdown(&self) {
        let _ = self.tx.try_send(HistoryAdapterCmd::Shutdown);
    }
}

impl std::fmt::Debug for HistoryAdapterHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoryAdapterHandle")
            .finish_non_exhaustive()
    }
}
