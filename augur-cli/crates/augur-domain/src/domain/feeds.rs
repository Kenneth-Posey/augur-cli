//! Feed domain types - typed message enums and structs for actor feed channels.
//!
//! Defines the message types that flow through the LLM feed, user input feed,
//! and history feed channels introduced in the actor-refactor feature.

// ── LlmFeedTag ───────────────────────────────────────────────────────────────

/// Classifies a single [`LlmFeedMessage`] chunk by its semantic origin.
///
/// Used by feed consumers to route chunks to the appropriate handler
/// (e.g., the TUI panel, history adapter, or tool executor).
#[derive(Debug, Clone, PartialEq)]
pub enum LlmFeedTag {
    /// A chunk from a background agent's LLM stream.
    BackgroundAgentChunk,
    /// LLM "thinking" tokens produced during internal reasoning.
    ThinkingChatter,
    /// The LLM wants to call a tool.
    ToolRequest,
    /// A token chunk directed at the user.
    UserChunk,
    /// A transport or parse error from the streaming layer.
    Error,
}

// ── UserInputTag ─────────────────────────────────────────────────────────────

/// Classifies a single [`UserFeedMessage`] by its input form.
///
/// Used by feed consumers to decide whether text should be forwarded raw or
/// processed as a structured command.
#[derive(Debug, Clone, PartialEq)]
pub enum UserInputTag {
    /// Raw text exactly as typed by the user.
    RawCommand,
    /// Structured parsed command ready for dispatch.
    ParsedCommand,
}

// ── LlmFeedMessage ───────────────────────────────────────────────────────────

/// A single tagged chunk flowing through an LLM feed channel.
///
/// Carries a [`LlmFeedTag`] identifying the chunk's semantic role alongside
/// the raw [`crate::domain::types::StreamChunk`] payload. Consumers inspect
/// `tag` to route the chunk without inspecting the payload directly.
#[derive(Debug, Clone)]
pub struct LlmFeedMessage {
    /// Semantic classification of the chunk.
    pub tag: LlmFeedTag,
    /// The underlying stream chunk from the LLM provider.
    pub chunk: crate::domain::types::StreamChunk,
}

// ── UserFeedMessage ───────────────────────────────────────────────────────────

/// A single tagged message flowing through a user-input feed channel.
///
/// Pairs a [`UserInputTag`] with the raw or parsed text so consumers can
/// decide whether further parsing is required.
#[derive(Debug, Clone)]
pub struct UserFeedMessage {
    /// Semantic classification of the user input.
    pub tag: UserInputTag,
    /// The text content of the user input.
    pub text: crate::domain::string_newtypes::OutputText,
}

// ── HistoryFeedMessage ───────────────────────────────────────────────────────

/// A single entry flowing through a history adapter feed channel.
///
/// Distinguishes user-originated messages from LLM-originated messages so
/// the history adapter can store them in the correct conversation slot.
#[derive(Debug, Clone)]
pub enum HistoryFeedMessage {
    /// A message produced by the user.
    UserEntry(crate::domain::types::Message),
    /// A message produced by the LLM.
    LlmEntry(crate::domain::types::Message),
}
