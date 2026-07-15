//! Session persistence data types.
//!
//! Defines the full data model for a saved session: identity metadata, message
//! records with explicit type tags, and summary projections.
//! All types derive `Serialize`/`Deserialize` for JSON round-trips via `serde_json`.

pub use crate::domain::types::{MessageRecord, MessageType};

use crate::domain::IsPredicate;
use crate::domain::newtypes::{Count, NumericNewtype, TimestampMs};
use crate::domain::string_newtypes::{
    EndpointName, OutputText, PromptText, SdkSessionId, SessionId, StrategyNodeName, StringNewtype,
};

/// Flags that further describe a persisted session.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionMetaFlags {
    /// Copilot SDK session identifier linked to this conversation.
    pub sdk_session_id: Option<SdkSessionId>,
    /// Whether the session was spawned from the ask panel.
    pub ask_session: IsPredicate,
}

/// Metadata stored alongside a persisted session record.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionMeta {
    /// Stable session identifier.
    pub id: SessionId,
    /// Creation timestamp for the session.
    pub created_at: TimestampMs,
    /// Last update timestamp for the session.
    pub last_updated_at: TimestampMs,
    /// Human-readable endpoint name for the session.
    pub endpoint_name: EndpointName,
    /// Additional session flags.
    #[serde(default)]
    pub flags: SessionMetaFlags,
    /// Session title, automatically set to the first user prompt (trimmed to 100 chars)
    /// or manually overridden via `/session-title` command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<OutputText>,
}

/// The current state of a persisted session.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionState {
    /// Stored messages in chronological order.
    #[serde(default)]
    pub messages: Vec<MessageRecord>,
    /// Persisted OpenRouter request-context history snapshot.
    #[serde(default)]
    pub openrouter_context_history: Option<Vec<crate::domain::types::Message>>,
    /// Active strategy tree for the session, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_strategy: Option<StrategyTree>,
}

/// A persisted session record.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionRecord {
    /// Persisted session metadata.
    pub meta: SessionMeta,
    /// Session state payload.
    pub state: SessionState,
}

/// Identity data for a session summary.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionIdentity {
    /// Session identifier.
    pub id: SessionId,
    /// Creation timestamp for the session.
    pub created_at: TimestampMs,
    /// Last update timestamp for the session.
    pub last_updated_at: TimestampMs,
    /// Human-readable endpoint name for the session.
    pub endpoint_name: EndpointName,
    /// Copilot SDK session identifier linked to this conversation.
    pub sdk_session_id: Option<SdkSessionId>,
    /// Whether the session was spawned from the ask panel.
    pub ask_session: IsPredicate,
    /// Session title. Set automatically from the first user prompt or
    /// overridden via `/session-title` command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<OutputText>,
}

/// Compact summary of a session suitable for listing.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionSummary {
    /// Identity of the session.
    pub identity: SessionIdentity,
    /// Number of messages in the session.
    pub message_count: Count,
    /// Preview text used by the session picker.
    pub preview: OutputText,
}

/// Convert a session record into a summary.
pub fn summarize(record: &SessionRecord) -> SessionSummary {
    SessionSummary {
        identity: SessionIdentity {
            id: record.meta.id.clone(),
            created_at: record.meta.created_at,
            last_updated_at: record.meta.last_updated_at,
            endpoint_name: record.meta.endpoint_name.clone(),
            sdk_session_id: record.meta.flags.sdk_session_id.clone(),
            ask_session: record.meta.flags.ask_session,
            title: record.meta.title.clone(),
        },
        message_count: Count::new(record.state.messages.len()),
        preview: record
            .state
            .messages
            .iter()
            .find(|msg| {
                msg.message_type == MessageType::User
                    && !msg.message.content.as_str().starts_with("[FILE:")
            })
            .map(|message| message.message.content.clone())
            .unwrap_or_else(|| OutputText::new("<<no prompt>>")),
    }
}

/// Metadata stored on a single node within a `StrategyTree`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NodeMeta {
    /// Node name key, mirroring the map key it lives under.
    pub name: StrategyNodeName,
    /// Human-readable description of what this node does.
    pub description: OutputText,
    /// Timestamp when this node was created.
    pub created_at: TimestampMs,
    /// Timestamp when this node was last modified.
    pub last_updated_at: TimestampMs,
    /// Timestamp when execution of this node finished, if it has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<TimestampMs>,
}

impl NodeMeta {
    /// Create a new `NodeMeta` with timestamps set to now and `finished_at` unset.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = TimestampMs::now();
        NodeMeta {
            name: StrategyNodeName::new(name),
            description: OutputText::new(description),
            created_at: now,
            last_updated_at: now,
            finished_at: None,
        }
    }
}

/// A node within a `StrategyTree`, either a terminal prompt or a branch to children.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StrategyNode {
    /// Metadata describing this node.
    pub meta: NodeMeta,
    /// Payload: a terminal prompt or a subtree of named child nodes.
    pub kind: StrategyNodeKind,
}

/// Describes whether a `StrategyNode` is a leaf with a prompt or a branch with children.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum StrategyNodeKind {
    /// Terminal node carrying a prompt to send to the LLM.
    Leaf(PromptText),
    /// Intermediate node whose children are further strategy nodes.
    Branch(std::collections::HashMap<StrategyNodeName, StrategyNode>),
}

/// A tree of strategy nodes, keyed by `StrategyNodeName`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct StrategyTree {
    /// All nodes in the tree, keyed by their unique name.
    pub nodes: std::collections::HashMap<StrategyNodeName, StrategyNode>,
}
