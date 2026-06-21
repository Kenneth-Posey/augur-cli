//! String-valued domain newtypes.
//!
//! Defines the `StringNewtype` trait and the `newtype_string!` generator macro.
//! Each generated type is a distinct wrapper around `String` so that different
//! semantic string concepts (model name, endpoint URL, tool name, etc.) cannot
//! be accidentally interchanged at call sites.

use crate::domain::newtypes::{Count, NumericNewtype, TextCharacter};
use std::fmt;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

/// Common interface shared by all string newtype wrappers.
///
/// Provides uniform construction, borrowing, and ownership-transfer operations.
/// Use as a bound in generic functions that must accept any semantic string type.
pub trait StringNewtype: Clone + Eq + Hash + fmt::Display {
    /// Wrap any value that converts to `String`.
    fn new(val: impl Into<String>) -> Self;
    /// Borrow the inner string slice. Equivalent to calling `Deref`.
    fn as_str(&self) -> &str;
    /// Consume the wrapper, returning the owned `String`.
    fn into_inner(self) -> String;
}

/// Generate a string-backed semantic newtype.
///
/// Produces a tuple struct with a private `String` field. Derives
/// `Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize`
/// (transparent serde). Implements `StringNewtype`, `Deref<Target=str>`,
/// `Display`, `From<String>`, and `From<&str>`.
macro_rules! newtype_string {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Clone, Debug,
            PartialEq, Eq, PartialOrd, Ord, Hash,
            serde::Serialize, serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl StringNewtype for $name {
            #[inline] fn new(val: impl Into<String>) -> Self { $name(val.into()) }
            #[inline] fn as_str(&self) -> &str { &self.0 }
            #[inline] fn into_inner(self) -> String { self.0 }
        }

        impl Deref for $name {
            type Target = str;
            #[inline] fn deref(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl From<String> for $name {
            #[inline] fn from(s: String) -> Self { $name(s) }
        }
        impl From<&str> for $name {
            #[inline] fn from(s: &str) -> Self { $name(s.to_owned()) }
        }
        impl PartialEq<&str> for $name {
            #[inline] fn eq(&self, other: &&str) -> bool { self.0 == *other }
        }
        impl PartialEq<$name> for &str {
            #[inline] fn eq(&self, other: &$name) -> bool { *self == other.0 }
        }
        impl PartialEq<String> for $name {
            #[inline] fn eq(&self, other: &String) -> bool { &self.0 == other }
        }
        impl PartialEq<$name> for String {
            #[inline] fn eq(&self, other: &$name) -> bool { self == &other.0 }
        }
    };
}

newtype_string!(
    /// LLM model identifier, e.g. `"gpt-4o"` or `"claude-opus-4-6"`.
    ModelName
);

newtype_string!(
    /// Base URL for an API endpoint, e.g. `"https://api.openai.com/v1"`.
    EndpointUrl
);

newtype_string!(
    /// Human-readable config key identifying an endpoint, e.g. `"openai-gpt4o"`.
    EndpointName
);

newtype_string!(
    /// Unique tool identifier used in LLM tool schemas, e.g. `"shell_exec"`.
    ToolName
);

newtype_string!(
    /// Human-readable tool description sent to the LLM with a tool schema.
    ToolDescription
);

newtype_string!(
    /// UUID string identifying a conversation session.
    SessionId
);

newtype_string!(
    /// User-entered prompt text submitted to the agent.
    PromptText
);

/// Mutable prompt input buffer used by the TUI editor.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct PromptBuffer(String);

impl StringNewtype for PromptBuffer {
    #[inline]
    fn new(val: impl Into<String>) -> Self {
        PromptBuffer(val.into())
    }
    #[inline]
    fn as_str(&self) -> &str {
        &self.0
    }
    #[inline]
    fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for PromptBuffer {
    type Target = String;
    #[inline]
    fn deref(&self) -> &String {
        &self.0
    }
}

impl DerefMut for PromptBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut String {
        &mut self.0
    }
}

impl fmt::Display for PromptBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for PromptBuffer {
    #[inline]
    fn from(s: String) -> Self {
        PromptBuffer(s)
    }
}

impl From<&str> for PromptBuffer {
    #[inline]
    fn from(s: &str) -> Self {
        PromptBuffer(s.to_owned())
    }
}

newtype_string!(
    /// Agent or LLM output text; also used for tool result content.
    OutputText
);

newtype_string!(
    /// User-selectable choice text shown in the `query_user` overlay.
    ChoiceText
);

newtype_string!(
    /// Name of a branch or child node within a persisted strategy tree.
    ///
    /// Used as the `HashMap` key for both `StrategyTree::root` and nested
    /// `StrategyNodeKind::Branch` children.
    StrategyNodeName
);

newtype_string!(
    /// Filesystem path used by file-read/file-write tools and `@`-attachment tokens.
    ///
    /// Holds a relative or absolute path string. Used by `FileScannerActor` for
    /// completion results and by the submit pipeline to build `UserMessageAttachment`
    /// entries for the Copilot SDK.
    FilePath
);

newtype_string!(
    /// Human-readable file name shown in completion rows (usually basename).
    FileDisplayName
);

newtype_string!(
    /// Human-formatted model label displayed in the TUI status bar and response headers.
    ModelLabel
);

newtype_string!(
    /// Human-readable active task label displayed in the agent feed panel.
    TaskName
);

newtype_string!(
    /// Human-readable status text shown in thinking indicators and status rows.
    StatusLabel
);

newtype_string!(
    /// Git branch display string shown in the status bar.
    GitBranch
);

newtype_string!(
    /// Working-directory display string shown in the status bar.
    WorkingDir
);

newtype_string!(
    /// Clipboard-ready text extracted from a rendered selection in the primary feed.
    SelectedText
);

newtype_string!(
    /// Shell command string passed to the shell_exec tool.
    ShellCommand
);

newtype_string!(
    /// Unique identifier for a node within a plan tree.
    ///
    /// Used as the primary key in depth-first tree traversal and as the
    /// step-file name stem (e.g. `"steps/{id}.md"`).
    PlanNodeId
);

newtype_string!(
    /// Unique identifier for a plan tree.
    ///
    /// Used as the subdirectory name on disk (`plans/{id}/`) and as the root
    /// branch node id when a new tree is constructed.
    PlanTreeId
);

newtype_string!(
    /// File name of a persisted step document within a plan's `steps/` directory.
    ///
    /// Used by `PlanTreeStore::write_step` and `PlanTreeStore::read_step`.
    StepFileName
);

newtype_string!(
    /// Full textual content persisted to or loaded from a plan step file.
    StepContent
);

newtype_string!(
    /// Full UTF-8 file contents stored in cache snapshots.
    CachedFileContent
);

newtype_string!(
    /// Copilot SDK session identifier returned by the SDK after session creation or resume.
    SdkSessionId
);

newtype_string!(
    /// Copilot SDK model identifier, e.g. `"claude-sonnet-4"` or `"gpt-4o"`.
    ModelId
);

newtype_string!(
    /// Unique identifier for a phase within a guided plan file.
    ///
    /// Maps directly to the `id` field in the YAML frontmatter of a plan file.
    PlanPhaseId
);

newtype_string!(
    /// Unique identifier for a deterministic orchestrator stage.
    WorkflowStageId
);

newtype_string!(
    /// Unique identifier for a deterministic orchestrator step.
    WorkflowStepId
);

newtype_string!(
    /// Thinking-depth label declared by the deterministic orchestrator workflow.
    WorkflowThinkingDepth
);

newtype_string!(
    /// Raw agent signal string consumed by deterministic signal normalization.
    WorkflowSignalValue
);

newtype_string!(
    /// Opaque identifier for a conversation session, backed by a UUID v4 string.
    ConversationId
);

newtype_string!(
    /// A display-safe name for a background agent shown in the agent feed panel.
    AgentName
);

newtype_string!(
    /// SDK-assigned identifier correlating tool execution events.
    ToolCallId
);

newtype_string!(
    /// Human-readable display label for an effort tier (e.g. `"low"`, `"high"`).
    EffortLabel
);

newtype_string!(
    /// High-level user goal text submitted to the supervisor meta-planner.
    GoalText
);

newtype_string!(
    /// Environment variable name that stores an API key or token.
    EnvVarName
);

newtype_string!(
    /// API key as configured directly in endpoint configuration.
    ApiKey
);

newtype_string!(
    /// Resolved API key value ready for request authentication.
    ApiKeyValue
);

newtype_string!(
    /// Bearer token value sent in an Authorization header.
    BearerToken
);

newtype_string!(
    /// Human-readable phase display name in a guided plan.
    PhaseName
);

newtype_string!(
    /// Human-readable plan display name in a guided plan.
    PlanName
);

newtype_string!(
    /// Reason a reviewer requested rework for a phase or hook.
    ReworkReason
);

newtype_string!(
    /// Reason a phase, plan, or hook failed.
    FailureReason
);

newtype_string!(
    /// Protocol version identifier (e.g., "v1", "v2", "v3").
    ///
    /// Represents a semantic version string for protocol compatibility tracking.
    /// Used in session initialization to track protocol evolution.
    ProtocolVersion
);

newtype_string!(
    /// Structured checkpoint identifier for session recovery.
    ///
    /// Uniquely identifies a saved session checkpoint for restoration.
    /// Used by checkpoint and recovery operations.
    CheckpointId
);

newtype_string!(
    /// Snapshot state hint or prior session state for recovery validation.
    ///
    /// Contains a description or serialized snapshot of the prior session state
    /// to aid in recovery validation and debugging.
    StateHint
);

newtype_string!(
    /// Rewind reason explaining why a session is being rewound to a checkpoint.
    ///
    /// Human-readable reason string such as "user request", "error recovery", etc.
    RewindReason
);

newtype_string!(
    /// Hook identifier describing an infrastructure callback (e.g., "before_turn").
    ///
    /// Identifies a specific hook within the session infrastructure.
    HookId
);

newtype_string!(
    /// Skill name or identifier for domain-level skill invocations.
    ///
    /// Used to identify which skill was invoked as part of agent coordination.
    SkillName
);

newtype_string!(
    /// Semantic wrapper for streaming content deltas to accumulate in the background feed.
    ///
    /// Represents a portion of streamed content (e.g., an `AssistantMessageDelta`)
    /// that is accumulated in `DeltaAccumulator` and flushed when reaching a threshold.
    /// Prevents accidental mixing with other string types like tool names or descriptions.
    ContentDelta
);

newtype_string!(
    /// Semantic wrapper for display line text ready to emit to background feed.
    ///
    /// Represents a formatted, ready-to-display line (e.g., tool execution summary)
    /// that is routed to the background event feed or UI. Prevents accidental mixing
    /// with other string types like raw content or intermediate calculations.
    DisplayLine
);

newtype_string!(
    /// Session initialization context containing additional configuration or metadata.
    ///
    /// Carries initialization parameters such as system prompt hints or config flags
    /// that are specific to session startup.
    InitContext
);

newtype_string!(
    /// Serialized JSON payload used by tool requests and external call envelopes.
    JsonPayload
);

newtype_string!(
    /// Accumulated text content from delta streaming operations.
    ///
    /// Represents accumulated text that will be flushed as a complete unit.
    AccumulatedText
);

newtype_string!(
    /// Resource being accessed in a permission request (file path, API endpoint, etc).
    ///
    /// Describes the resource that permission is being requested for.
    ResourceId
);

newtype_string!(
    /// Permission type categorizing the kind of access being requested (read, write, etc).
    ///
    /// Examples: "read", "write", "execute", "delete".
    PermissionType
);

newtype_string!(
    /// Reason text explaining why a permission is needed.
    ///
    /// Human-readable explanation for audit and user understanding.
    PermissionReason
);

newtype_string!(
    /// Semantic identifier for a SessionEventData variant type.
    /// Examples: "ToolExecutionStart", "SessionError", "AssistantMessageDelta"
    EventType
);

newtype_string!(
    /// Evaluation pass criterion text forwarded to agent dispatch prompts.
    ///
    /// Represents a single criterion string that a worker or evaluator agent must
    /// satisfy for a workflow step to be marked as passed.
    PassCriterion
);

newtype_string!(
    /// Optional free-form feature context text forwarded to agent dispatch prompts.
    ///
    /// Contains the combined user message and attachment content used to provide
    /// background context to worker and evaluator agents during pipeline execution.
    FeatureContext
);

newtype_string!(
    /// Derived feature slug used as a filesystem path component.
    ///
    /// A lowercase, hyphen-joined slug derived from the first five words of the
    /// feature request text. Used to substitute `<feature-slug>` placeholders in
    /// workflow artifact paths.
    FeatureSlug
);

newtype_string!(
    /// Serialized parameter list for a function signature.
    ///
    /// Captures the full parameter string as extracted from source code or metadata,
    /// e.g. `"self, name: &str, count: usize"`.
    ParamList
);

newtype_string!(
    /// Return type string for a function signature.
    ///
    /// Captures the return type as a string, e.g. `"i32"`, `"Option<String>"`, `"()"`.
    ReturnTypeStr
);

newtype_string!(
    /// Generic parameter clause for a function signature.
    ///
    /// Contains the generics string extracted from the function declaration,
    /// e.g. `"<T: Clone, U>"`. Empty string when no generics are present.
    GenericParams
);

newtype_string!(
    /// Optional semantic label attached to a call edge.
    ///
    /// Describes the role or intent of the call relationship, e.g. `"delegate"`,
    /// `"adapter"`, `"impl_detail"`. Empty when no hint was supplied.
    SemanticHint
);

newtype_string!(
    /// RFC 3339 timestamp recording when a call graph was built.
    ///
    /// Stored as the ISO-8601 / RFC 3339 string returned by `chrono::Local::now().to_rfc3339()`.
    GraphTimestamp
);

newtype_string!(
    /// Documentation comment string attached to a graph node.
    ///
    /// Contains the extracted doc comment text for the corresponding function,
    /// or an empty string when no documentation is present.
    DocString
);

newtype_string!(
    /// Normalized function name produced by a chain-collapse consolidation operation.
    ///
    /// Represents the merged identifier that replaces a linear chain of single-caller /
    /// single-callee functions after the collapse transformation.
    MergedFunctionName
);

newtype_string!(
    /// Human-readable rationale explaining a consolidation opportunity.
    ///
    /// Provides context for why a given consolidation action was suggested,
    /// surfaced in reports and user-facing output.
    Rationale
);

// --- String newtypes for Phase 2 primitive cleanup ---

newtype_string!(
    /// Intent or skill name in task runner and execution specs.
    ///
    /// Identifies the specific intent or skill that a task step should execute,
    /// preventing accidental confusion with other string identifiers like
    /// tool names or plan node IDs.
    IntentName
);

newtype_string!(
    /// Log entry role label (e.g. "user", "assistant", "system").
    ///
    /// Distinguishes the role associated with a log entry from other string
    /// values like endpoint names or content text.
    RoleLabel
);

newtype_string!(
    /// Log entry content text.
    ///
    /// Contains the payload of a log entry, distinguished from other string
    /// values like role labels or endpoint names.
    LogContent
);

newtype_string!(
    /// LSP workspace root URI string.
    ///
    /// Represents the root URI for an LSP workspace, preventing accidental
    /// confusion with file paths or other URI strings.
    RootUri
);

newtype_string!(
    /// Serialized execution step specification JSON.
    ///
    /// Contains the JSON-serialized spec for an execution step, preventing
    /// accidental confusion with other string values like artifact data or
    /// file content.
    StepSpecJson
);

newtype_string!(
    /// Name of a persisted step artifact.
    ///
    /// Identifies an artifact by name within a step's output, preventing
    /// accidental confusion with artifact data or other string identifiers.
    ArtifactName
);

newtype_string!(
    /// Data/payload of a persisted step artifact.
    ///
    /// Contains the actual data produced by a step, distinguished from
    /// the artifact name or other string values.
    ArtifactData
);

newtype_string!(
    /// Provider identifier string in the catalog (e.g. "openai", "anthropic").
    ///
    /// Identifies the provider responsible for serving an endpoint model,
    /// preventing accidental confusion with endpoint names or model names.
    ProviderName
);

newtype_string!(
    /// Shell command process ID string.
    ///
    /// Contains the string representation of a process ID from shell execution,
    /// preventing accidental confusion with other string identifiers.
    ProcessId
);

newtype_string!(
    /// Label text shown in the TUI spinner.
    ///
    /// Display text for a spinner animation in the terminal UI, distinguished
    /// from other label types like status labels or model labels.
    SpinnerLabel
);

newtype_string!(
    /// Tools description text in command registry.
    ///
    /// Contains a formatted description of available tools for display or
    /// serialization, preventing confusion with tool names or descriptions.
    ToolsText
);

newtype_string!(
    /// Key identifier for a dynamic control item.
    ///
    /// Used to identify a control item in the TUI dynamic controls panel,
    /// preventing accidental confusion with labels or other string values.
    ControlKey
);

newtype_string!(
    /// Label text for a dynamic control item.
    ///
    /// Display label shown in the TUI dynamic controls panel, distinguished
    /// from control keys or other label types.
    ControlLabel
);

impl ConversationId {
    /// Create a new unique conversation identifier via `uuid::Uuid::new_v4()`.
    ///
    /// Each call produces a distinct UUID v4 value. This is the only correct
    /// construction site for new identifiers - do not fabricate UUIDs elsewhere.
    pub fn generate() -> Self {
        ConversationId(uuid::Uuid::new_v4().to_string())
    }
}

impl OutputText {
    /// Append a character to the end of this text buffer.
    pub fn push(&mut self, ch: TextCharacter) {
        self.0.push(ch.0);
    }

    /// Remove and return the final character from this text buffer, if any.
    pub fn pop(&mut self) -> Option<TextCharacter> {
        self.0.pop().map(TextCharacter)
    }

    /// Append another output-text fragment to the end of this buffer.
    pub fn push_output(&mut self, text: &OutputText) {
        self.0.push_str(text.as_str());
    }

    /// Return the byte index after the first `chars` Unicode scalar values.
    pub fn prefix_byte_end(&self, chars: Count) -> Count {
        Count::new(
            self.0
                .char_indices()
                .nth(chars.inner())
                .map(|(idx, _)| idx)
                .unwrap_or(self.0.len()),
        )
    }

    /// Drain and return the prefix ending at `byte_end`.
    pub fn drain_prefix(&mut self, byte_end: Count) -> OutputText {
        OutputText(self.0.drain(..byte_end.inner()).collect())
    }

    /// Move all buffered text out, leaving this value empty.
    pub fn take_all(&mut self) -> OutputText {
        std::mem::replace(self, OutputText::from(""))
    }
}

impl PromptText {
    /// Append a character to the end of this prompt buffer.
    pub fn push(&mut self, ch: TextCharacter) {
        self.0.push(ch.0);
    }

    /// Remove and return the final character from this prompt buffer, if any.
    pub fn pop(&mut self) -> Option<TextCharacter> {
        self.0.pop().map(TextCharacter)
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        ConversationId::generate()
    }
}

impl Default for ModelLabel {
    fn default() -> Self {
        Self::new("")
    }
}

impl Default for StatusLabel {
    fn default() -> Self {
        Self::new("")
    }
}

impl Default for WorkingDir {
    fn default() -> Self {
        Self::new("")
    }
}
