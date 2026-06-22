//! Event Mapper Domain: 11 semantic types + inventory mapping for Copilot SDK event output.
//!
//! This module defines the semantic domain types that represent distinct event kinds
//! produced by Copilot SDK sessions. Each type is semantically independent (no internal
//! cross-references in Phase 1) and carries distinct metadata that cannot be represented
//! by existing `Message`, `ToolCall`, or `AgentOutput` types.
//!
//! It also provides the complete event inventory mapping, categorizing all 41 SessionEventData
//! variants and their routing destinations.
//!
//! See:
//! - `plans-ecosystem/04-27-2026-1645-event-mapper-domain-stage-part-01-reuse-audit.md` for type justifications
//! - `plans-ecosystem/04-27-2026-1645-event-mapper-domain-stage-part-02-domain-inventory.md` for mapping decisions

pub mod contracts;
pub mod inventory;
pub mod protocols;

use crate::domain::newtypes::TimestampMs;
use crate::domain::string_newtypes::{
    AgentName, CheckpointId, ContentDelta, EndpointUrl, FeatureContext, HookId, InitContext,
    JsonPayload, ModelId, PermissionReason, PermissionType, ProtocolVersion, ResourceId,
    RewindReason, SessionId, SkillName, StateHint, ToolName,
};
use crate::domain::{ErrorMessage, ExecutionSuccess, IsPredicate, WaitSecs};

/// Session metadata and configuration.
///
/// Represents the initialization parameters and model information for a session.
/// Distinct from `ContextUsageStats` (which carries only live token counts) because
/// `SessionInfo` must include model, protocol_version, and session-wide metadata not
/// tied to token accounting.
///
/// **Semantic Role**: Session initialization event; emitted when a new session begins
/// or is resumed from checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionInfo {
    /// LLM model identifier (e.g., `"gpt-4o"` or `"claude-opus-4-6"`).
    pub model: ModelId,
    /// Protocol version for this session (e.g., `"v3"` or `"v4"`).
    /// Tracks protocol evolution and session compatibility.
    pub protocol_version: ProtocolVersion,
    /// Human-readable session identifier. Uniquely identifies the session within the system.
    pub session_id: SessionId,
    /// Unix timestamp (milliseconds) when session was initiated.
    pub timestamp: TimestampMs,
}

/// Session initialization event with context.
///
/// Emitted when a new Copilot SDK session begins processing. Carries initialization
/// context distinct from a generic `Message` because lifecycle events are state
/// machines (with defined transitions), not free-form user/assistant exchanges.
/// No existing type captures both session initialization context and timestamp semantics.
///
/// **Semantic Role**: Marks the start of a new session lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionStarted {
    /// Session metadata (model, protocol version, ID).
    pub session_info: SessionInfo,
    /// Additional initialization context (e.g., system prompt hints, config flags).
    /// Contains structured or semi-structured initialization parameters.
    pub init_context: InitContext,
}

/// Session recovery event after checkpoint restoration.
///
/// Emitted when a session resumes from a saved checkpoint. Distinct from `SessionStarted`
/// because recovery context (prior session state, checkpoint identity) is not needed
/// for new sessions. Cannot compose with generic `Message` without muddying intent.
///
/// **Semantic Role**: Marks session recovery after interruption or persistence restore.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionResumed {
    /// Session metadata.
    pub session_info: SessionInfo,
    /// Identifier of the checkpoint being restored.
    pub checkpoint_id: CheckpointId,
    /// Snapshot of prior session state for recovery validation.
    /// Used for audit trail and recovery verification.
    pub prior_state_hint: StateHint,
}

/// Checkpoint restore operation with rewind semantics.
///
/// Represents a snapshot checkpoint identity and the rewind operation itself.
/// Distinct from usage stats because this carries checkpoint identity + rewind
/// semantics, not token accounting. No existing type represents "restore to checkpoint"
/// operations with rollback metadata.
///
/// **Semantic Role**: Marks a request to rewind the session to a prior checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SnapshotRewind {
    /// Identifier of the target checkpoint.
    pub checkpoint_id: CheckpointId,
    /// Unix timestamp (milliseconds) when checkpoint was created.
    pub checkpoint_created_at: TimestampMs,
    /// Human-readable reason for rewind (e.g., "user request", "error recovery").
    pub reason: RewindReason,
}

/// Extended thinking / reasoning data stream.
///
/// Carries internal reasoning or "chain of thought" data that may be hidden from
/// users or displayed separately. Distinct from `Message` (role=Assistant) because
/// extended thinking is not user-facing text and is computed in a separate pipeline.
/// Merging it into `Message` would conflate prompt responses with internal reasoning.
///
/// **Semantic Role**: Streaming intermediate reasoning or thinking steps during
/// model inference.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Reasoning {
    /// Partial reasoning text chunk from streaming response.
    pub reasoning_text: ContentDelta,
    /// Unix timestamp (milliseconds) when this chunk was received.
    pub timestamp: TimestampMs,
}

/// Tool invocation request with permission/approval semantics.
///
/// Distinct from `ToolCall` (name + args only) because this carries permission and
/// approval semantics from Protocol v3. Not composable without losing type safety
/// of structured approval workflows.
///
/// **Semantic Role**: Represents a tool invocation that requires permission or
/// tracking before execution.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolRequested {
    /// Tool name as identified by the SDK.
    pub tool_name: ToolName,
    /// Serialized tool arguments (JSON).
    pub arguments_json: JsonPayload,
    /// Whether user approval is required before execution.
    /// `true` = approval required, `false` = can execute immediately.
    pub requires_approval: IsPredicate,
    /// Unix timestamp (milliseconds) when request was made.
    pub timestamp: TimestampMs,
}

/// External tool call with addressing, authentication, and timeout.
///
/// External tools have distinct addressing (endpoint URL, auth tokens, timeout)
/// vs. internal `ToolCall` (name + args only). Not composable without losing
/// security and network-level type safety.
///
/// **Semantic Role**: Represents an out-of-process or remote tool invocation
/// with network and security metadata.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternalToolRequest {
    /// Tool endpoint URL or identifier.
    pub endpoint: EndpointUrl,
    /// Serialized request payload (JSON).
    pub request_payload: JsonPayload,
    /// Timeout in seconds for the external call.
    /// Used to prevent indefinite waiting on remote service calls.
    pub timeout_secs: WaitSecs,
}

/// Permission/authorization request with structured approval flow.
///
/// Permissions are not system messages; they require structured approval workflows
/// (resource, permission type, grant/deny outcome). `SystemMessage` is unstructured text.
/// This type enforces UI workflow and audit requirements distinct from generic messages.
///
/// **Semantic Role**: Represents a request for user/admin permission to perform an
/// operation on a resource.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PermissionRequest {
    /// Resource being accessed (e.g., file path, API endpoint, database).
    pub resource: ResourceId,
    /// Permission type (e.g., "read", "write", "execute", "delete").
    pub permission_type: PermissionType,
    /// Human-readable description of why permission is needed.
    /// Used for user understanding and audit trails.
    pub reason: PermissionReason,
}

/// Hook invocation start event.
///
/// Hooks are infrastructure callbacks distinct from tool calls (no args, no result).
/// No existing type captures hook identity + invocation semantics. Future-proofed
/// for hook registry and infrastructure event tracking.
///
/// **Semantic Role**: Marks the start of an infrastructure hook (e.g., before-turn,
/// after-tool-call).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HookStarted {
    /// Hook identifier (e.g., `"before_turn"`, `"after_tool_exec"`).
    /// Uniquely identifies the hook within the session infrastructure.
    pub hook_id: HookId,
    /// Unix timestamp (milliseconds) when hook was invoked.
    pub timestamp: TimestampMs,
}

/// Hook completion event with context tracking.
///
/// Hook completion is not tool completion (no result parsing, no error propagation).
/// Requires hook context tracking separate from `ToolCallCompleted`. Infrastructure-specific
/// semantics for observability and debugging.
///
/// **Semantic Role**: Marks the completion of a hook invocation, with success/failure
/// status.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HookCompleted {
    /// Hook identifier.
    pub hook_id: HookId,
    /// Whether the hook executed successfully.
    /// `true` = executed successfully, `false` = hook failed during execution.
    pub success: ExecutionSuccess,
    /// Optional error message if the hook failed.
    /// Present when `success = false`; typically `None` when successful.
    pub error_message: Option<ErrorMessage>,
}

/// Skill invocation event with agent and metadata.
///
/// Skills are domain-level invocations distinct from tools and hooks. No existing type
/// represents skill metadata + agent context. Part of larger skills framework for
/// coordinating multi-agent behavior.
///
/// **Semantic Role**: Marks the invocation of a skill, which may invoke one or more
/// tools or other agents.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillInvoked {
    /// Skill name or identifier.
    pub skill_name: SkillName,
    /// Agent that invoked the skill.
    pub invoked_by: AgentName,
    /// Optional context or parameters passed to the skill.
    /// Contains skill-specific configuration or metadata.
    pub context: FeatureContext,
    /// Unix timestamp (milliseconds) when skill was invoked.
    pub timestamp: TimestampMs,
}
