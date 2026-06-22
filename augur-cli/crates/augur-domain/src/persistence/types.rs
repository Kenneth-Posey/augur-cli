//! Session persistence data types.
//!
//! Defines the full data model for a saved session: identity metadata, message
//! records with explicit type tags, strategy trees, and summary projections.
//! All types derive `Serialize`/`Deserialize` for JSON round-trips via `serde_json`.

use std::collections::HashMap;

pub use crate::domain::types::{MessageRecord, MessageType};

use crate::domain::IsPredicate;
use crate::domain::newtypes::{Count, NumericNewtype, TimestampMs};
use crate::domain::string_newtypes::{
    EndpointName, OutputText, PromptText, SdkSessionId, SessionId, StrategyNodeName, StringNewtype,
};

// ── Strategy tree ────────────────────────────────────────────────────────────

/// Metadata attached to every node in a `StrategyTree`.
///
/// Tracks name, description, and three timestamps: creation, last update, and
/// optional finish time. `NodeMeta::new` stamps `created_at` and
/// `last_updated_at` to the current wall clock; `finished_at` starts as `None`
/// and is set when the node's work is complete.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NodeMeta {
    /// Human-readable label for this strategy node.
    pub name: OutputText,
    /// Description of the node's purpose or scope.
    pub description: OutputText,
    /// Wall-clock timestamp of node creation.
    pub created_at: TimestampMs,
    /// Wall-clock timestamp of the most recent update to this node.
    pub last_updated_at: TimestampMs,
    /// Wall-clock timestamp when this node's work was finished; `None` if still active.
    pub finished_at: Option<TimestampMs>,
}

impl NodeMeta {
    /// Create a new `NodeMeta` with both timestamps set to now and no finish time.
    pub fn new(name: impl Into<OutputText>, description: impl Into<OutputText>) -> Self {
        let now = TimestampMs::now();
        NodeMeta {
            name: name.into(),
            description: description.into(),
            created_at: now,
            last_updated_at: now,
            finished_at: None,
        }
    }
}

/// The kind of a strategy node: either a branch containing child nodes or a
/// leaf containing a final prompt string.
///
/// `Branch` nodes hold named children that can themselves be branches or
/// leaves, forming a tree. `Leaf` holds the terminal prompt string used when
/// that branch of the strategy is reached.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum StrategyNodeKind {
    /// Intermediate node; maps child names to their `StrategyNode` entries.
    Branch(HashMap<StrategyNodeName, StrategyNode>),
    /// Terminal node containing the final prompt string for this strategy path.
    Leaf(PromptText),
}

/// A single node in a `StrategyTree`, combining metadata with its kind.
///
/// Every node carries a `NodeMeta` regardless of depth so that timing and
/// labelling information is available at any level of the tree.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StrategyNode {
    /// Metadata describing this node.
    pub meta: NodeMeta,
    /// Whether this node branches to children or holds a final prompt.
    pub kind: StrategyNodeKind,
}

/// A named tree of strategies, rooted at a `HashMap` of top-level nodes.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct StrategyTree {
    /// Top-level strategy nodes keyed by name.
    pub nodes: HashMap<StrategyNodeName, StrategyNode>,
}

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
    /// Persisted guided strategy tree.
    #[serde(default)]
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
        },
        message_count: Count::new(record.state.messages.len()),
        preview: record
            .state
            .messages
            .first()
            .map(|message| message.message.content.clone())
            .unwrap_or_else(|| OutputText::new("")),
    }
}
