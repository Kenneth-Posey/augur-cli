//! AppState: owns all mutable terminal UI state. No channels - plain owned data.

#[path = "tui_state/lifecycle.rs"]
mod lifecycle;
#[path = "tui_state/output_flow.rs"]
mod output_flow;
#[path = "tui_state/output_messages.rs"]
mod output_messages;

use augur_domain::domain::newtypes::{
    Count, IsActive, IsAwaitingCompact, IsPredicate, IsReviewActive, IsRunning, IsSeeded,
    IsThinking, IsTurnComplete, NumericNewtype, ScrollOffset, ShouldResetUsage, TimestampMs,
};
use augur_domain::domain::plan_tree::PlanTree;
use augur_domain::domain::string_newtypes::{
    ChoiceText, EndpointName, GitBranch, ModelId, ModelLabel, OutputText, PhaseName, PlanName,
    PromptBuffer, PromptText, SessionId, StatusLabel, StringNewtype, TaskName, WorkingDir,
};
use augur_domain::domain::types::{
    CommandDef, ContextUsageStats, FeedId, FileCompletion, ModelOption, ProjectTokenTotals,
};
use ratatui::layout::Rect;
use std::cell::Cell;
use std::ops::{Deref, DerefMut};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::oneshot;

/// Capture the current wall-clock time as a millisecond-precision `TimestampMs`.
///
/// Used when stamping user-submitted lines and response block starts with the
/// time they are first rendered. Falls back to zero when the system clock is
/// unavailable (should never occur in practice).
pub fn current_timestamp_ms() -> TimestampMs {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    TimestampMs::new(ms)
}

/// Stable identity and routing fields for a picker session row.
#[derive(Clone, Debug, bon::Builder)]
pub struct PickerSessionIdentity {
    /// Stable session identifier.
    pub id: SessionId,
    /// When the session was created.
    pub created_at: TimestampMs,
    /// When the session was last saved; used for newest-first sort in the picker.
    pub last_updated_at: TimestampMs,
    /// The LLM endpoint active in this session.
    pub endpoint_name: EndpointName,
}

/// Lightweight session projection owned by the shared TUI contract layer.
#[derive(Clone, Debug, bon::Builder)]
pub struct PickerSessionSummary {
    /// Stable session identity and routing data.
    pub identity: PickerSessionIdentity,
    /// Number of messages in the session.
    pub message_count: Count,
    /// Truncated preview text from the first user message.
    pub preview: OutputText,
}

/// State for the startup session picker screen.
#[derive(Clone)]
pub struct PickerState {
    /// Ordered list of sessions available to restore.
    pub sessions: Vec<PickerSessionSummary>,
    /// Index of the currently highlighted session in the list.
    pub selected: Count,
}

#[derive(bon::Builder)]
/// State for the query overlay shown when the LLM calls the `query_user` tool.
///
/// Holds the question, optional choices, current selection, free-form input buffer,
/// and the oneshot sender used to return the user's answer to the waiting tool task.
pub struct QueryState {
    /// The question text displayed at the top of the overlay.
    pub question: PromptText,
    /// Optional choices the user can navigate with up/down arrows.
    pub choices: Vec<ChoiceText>,
    /// Index of the currently highlighted choice, or `None` when no choice is selected.
    pub selected: Option<usize>,
    /// Free-form text the user has typed; takes priority over a selected choice on submit.
    pub freeform: PromptText,
    /// Oneshot sender; the TUI sends the resolved answer back through this channel.
    pub reply_tx: oneshot::Sender<OutputText>,
}

#[derive(bon::Builder)]
/// State specific to plan mode, holding the tree snapshot and panel scroll offset.
///
/// Only plan-mode-specific fields live here; the shared chat state (output, prompt,
/// agent, status) remains on `AppState` and is used by both `Chat` and `Plan` modes.
#[derive(Clone)]
pub struct PlanModeState {
    /// The current plan tree snapshot rendered in the right panel.
    pub tree: PlanTree,
    /// `false` = preview mode (tree shown but not running), `true` = executing.
    pub running: IsRunning,
    /// Scroll offset for the right plan panel. 0 shows the top of the tree.
    pub tree_scroll: ScrollOffset,
}

#[derive(bon::Builder)]
/// UI state for guided plan execution mode.
///
/// Holds the per-phase display data rendered in the right panel and flags
/// controlling the reviewer overlay. Owned by `ConversationMode::GuidedPlan`.
/// Consumers: `render_guided_plan`, `actors::tui::actor` (event handler).
#[derive(Clone)]
pub struct GuidedPlanUiState {
    /// Ordered list of (phase_name, status) pairs for right-panel rendering.
    pub phases: Vec<(PhaseName, augur_domain::domain::guided_plan::PhaseStatus)>,
    /// Zero-based index of the currently active phase.
    pub current_phase: usize,
    /// Human-readable plan name shown as the panel header.
    pub plan_name: PlanName,
    /// `true` while a Copilot agent hook is streaming reviewer tokens into
    /// the main chat. The renderer shows a `"Reviewer active…"` banner.
    pub review_active: IsReviewActive,
    /// `true` after `CompactRequested` fires: the TUI has called `agent.compact()`
    /// and is waiting for `AgentOutput::CompactionComplete` before signalling
    /// `GuidedPlanHandle::compaction_done()` to unblock the guided plan actor.
    pub guided_awaiting_compact: IsAwaitingCompact,
}

impl GuidedPlanUiState {
    /// Build a `GuidedPlanUiState` from a `GuidedPlanConfig`.
    ///
    /// All phases start as `Pending`. Called by the `/run-plan` command handler
    /// immediately after `load_guided_plan` succeeds.
    pub fn from_config(config: &augur_domain::domain::guided_plan::GuidedPlanConfig) -> Self {
        GuidedPlanUiState::builder()
            .phases(
                config
                    .phases
                    .iter()
                    .map(|p| {
                        (
                            PhaseName::new(p.name.to_string()),
                            augur_domain::domain::guided_plan::PhaseStatus::Pending,
                        )
                    })
                    .collect(),
            )
            .current_phase(0)
            .plan_name(PlanName::new(config.name.to_string()))
            .review_active(IsReviewActive::no())
            .guided_awaiting_compact(IsAwaitingCompact::no())
            .build()
    }
}

/// Outer full-screen context. Controls which top-level screen the shell renders.
///
/// `SessionSelector` is shown at startup when saved sessions are available.
/// `Conversation` is the main interaction screen.
///
/// Consumers: `AppInteraction`, `render`, `actors::tui::actor`, `picker`.
#[derive(Clone)]
pub enum AppScreen {
    /// Startup session picker screen; holds the list of candidate sessions.
    SessionSelector(PickerState),
    /// Full conversation screen: primary feed, text entry, and footer.
    Conversation,
}

/// Active mode within the conversation screen.
///
/// Only meaningful when `AppInteraction::screen` is `AppScreen::Conversation`.
/// Variants are mutually exclusive at runtime.
///
/// Consumers: `AppInteraction`, `render`, `key_dispatch`, `plan_view`.
pub enum ConversationMode {
    /// Normal chat interaction mode.
    Chat,
    /// Query overlay mode; the LLM is waiting for a structured user answer.
    Query(QueryState),
    /// Plan mode: chat on the left 75%, plan tree panel on the right 25%.
    Plan(PlanModeState),
    /// Guided plan execution mode: chat on the left 75%, phase panel on the right 25%.
    GuidedPlan(GuidedPlanUiState),
}

/// Which view is displayed in the secondary container panel.
///
/// At most one secondary view is visible at a time. `None` means the
/// secondary container is closed.
///
/// Consumers: `AppInteraction`, render (Phase 2+).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecondaryView {
    /// The ask side-channel panel (existing functionality).
    Ask,
    /// Live background task output feed (introduced in Phase 3).
    AgentFeed,
}

/// Live output from background tasks rendered in the agent feed panel.
///
/// Initialized on first open and persists for the lifetime of the TUI session.
/// `scroll == 0` means follow the latest output.
///
/// Consumers: `AppInteraction`, render (Phase 3).
/// Buffers for accumulating and batching event output lines.
///
/// Prevents interleaving of different event types (tool responses, status messages)
/// with streamed content. Each buffer is flushed on appropriate structural events.
#[derive(Default, Clone, bon::Builder)]
pub struct EventBuffers {
    /// Buffer for accumulating consecutive `StatusLine` events.
    ///
    /// When a `StatusLine` event arrives, it is appended to this buffer
    /// instead of being immediately pushed to output. This allows multiple
    /// consecutive messages to appear on a single line. The buffer is flushed
    /// to output when a task-end event (`TaskCompleted`, `TaskFailed`) or
    /// `Clear` event arrives, or when a structural event occurs.
    pub pending_status_message: Option<OutputLine>,
    /// Buffer for pending `ToolEventLine` to prevent interleaving with streamed messages.
    ///
    /// When a `ToolEventLine` event arrives, it is buffered instead of being
    /// immediately pushed to output. This prevents tool event lines from
    /// interleaving with `StatusLine` messages that are still being streamed.
    /// The buffer is flushed to output when a task-end event (`TaskCompleted`,
    /// `TaskFailed`), a structural event (`TaskStarted`), `Clear`, or
    /// `MessageBreak` (end of a streamed assistant message) arrives.
    pub pending_tool_event: Option<OutputLine>,
}

/// Rendered panel state shared by selected and per-feed transcripts.
#[derive(Clone, Default, bon::Builder)]
pub struct AgentFeedPanel {
    /// Accumulated output lines from background task events.
    pub output: Vec<OutputLine>,
    /// Scroll offset within the panel. 0 = follow latest.
    pub scroll: ScrollOffset,
    /// Buffers for batching event output to prevent interleaving.
    pub buffers: EventBuffers,
}

/// Transcript state for one background agent feed.
///
/// Each feed keeps its own output, scroll position, active-task metadata, and
/// batching buffers so parallel agent runs do not overwrite one another.
#[derive(Clone)]
pub struct AgentFeedTranscript {
    /// Stable feed identifier for this transcript.
    pub feed_id: FeedId,
    /// Rendered panel data for this feed.
    pub panel: AgentFeedPanel,
    /// Display name of the currently active task, or `None` when idle.
    pub active_task: Option<TaskName>,
    /// Model name of the currently running agent, or `None` when idle.
    pub current_agent_model: Option<ModelLabel>,
}

impl Default for AgentFeedTranscript {
    fn default() -> Self {
        Self {
            feed_id: FeedId::Agent(augur_domain::domain::string_newtypes::ToolCallId::from("")),
            panel: AgentFeedPanel {
                output: Vec::new(),
                scroll: ScrollOffset::default(),
                buffers: EventBuffers::default(),
            },
            active_task: None,
            current_agent_model: None,
        }
    }
}

impl Deref for AgentFeedTranscript {
    type Target = AgentFeedPanel;

    fn deref(&self) -> &Self::Target {
        &self.panel
    }
}

impl DerefMut for AgentFeedTranscript {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.panel
    }
}

#[derive(Default, Clone, bon::Builder)]
/// State backing the agent-feed side panel.
///
/// Tracks the selected feed's rendered output plus the full set of background
/// agent transcripts and selection state.
pub struct AgentFeedState {
    /// Rendered panel data for the selected feed.
    pub panel: AgentFeedPanel,
    /// Display name of the currently active task, or `None` when idle.
    pub active_task: Option<TaskName>,
    /// Model name of the currently running agent, or `None` when idle.
    /// Used for the agent feed panel title label (e.g., "[ claude-haiku-4.5 ]").
    pub current_agent_model: Option<ModelLabel>,
    /// All tracked background-agent transcripts in first-seen order.
    #[builder(default)]
    pub feeds: Vec<AgentFeedTranscript>,
    /// Index of the selected feed within `feeds`, when one is active.
    pub selected_feed: Option<usize>,
}

impl Deref for AgentFeedState {
    type Target = AgentFeedPanel;

    fn deref(&self) -> &Self::Target {
        &self.panel
    }
}

impl DerefMut for AgentFeedState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.panel
    }
}

/// Which input area currently receives keyboard input.
///
/// `Main` is the default and sends Enter-submissions to the main agent.
/// `Ask` routes Enter-submissions to the ask actor when the panel is open.
/// Consumers: `key_dispatch::handle_submit`, `apply_ask_output`, `render_ask_panel`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputFocus {
    /// The main chat input has focus. Default state.
    #[default]
    Main,
    /// The ask panel input has focus. Active while `ask_panel` is open.
    Ask,
}

/// State for the ask side-channel panel.
///
/// Holds the panel's output lines, scroll offset, thinking indicator, and a
/// seeded flag. The panel accumulates its own conversation independently of
/// the main chat.
///
/// Invariants:
/// - `seeded` transitions from `false` to `true` exactly once, when the main
///   conversation history snapshot has been injected via `RestoreSession`.
/// - `thinking` is `true` while the ask actor is processing a turn.
///     - Consumers: `render_ask_panel`, `apply_ask_output`, `handle_ask_submit`.
#[derive(Default, Clone, bon::Builder)]
pub struct AskPanelState {
    /// Accumulated output lines from ask turns.
    pub output: Vec<OutputLine>,
    /// Scroll offset within the ask panel. 0 means follow the latest output.
    pub scroll: ScrollOffset,
    /// True while the ask actor is processing a turn.
    pub thinking: IsThinking,
    /// True after the main history snapshot has been injected into the ask actor.
    pub seeded: IsSeeded,
}

#[derive(Clone, bon::Builder)]
/// Secondary-panel overlay state: ask panel, task feed, active secondary view,
/// and keyboard focus between the main and ask inputs.
pub struct PanelOverlayState {
    /// Ask panel overlay state. `None` when the panel is closed.
    pub ask_panel: Option<AskPanelState>,
    /// Agent feed state. Initialized to default and persists while app runs.
    pub agent_feed: AgentFeedState,
    /// Which secondary view is currently active. `None` = secondary closed.
    pub secondary_view: Option<SecondaryView>,
    /// Which input area currently has keyboard focus.
    pub input_focus: InputFocus,
}

#[derive(bon::Builder)]
/// Bundled interaction state: screen context, conversation mode, and panel overlays.
///
/// Groups all interactive state so `AppState` stays within the 5-field limit.
///
/// # Invariant
///
/// `mode: ConversationMode` is **only meaningful** when
/// `screen == AppScreen::Conversation`. When `screen` is
/// `AppScreen::SessionSelector`, `mode` is ignored by the renderer and event
/// handlers - it defaults to `ConversationMode::Chat` and must not be read.
///
/// Consumers: `AppState`, `key_dispatch`, `render`, `apply_ask_output`.
pub struct AppInteraction {
    /// Current full-screen context: session selector or conversation.
    pub screen: AppScreen,
    /// Active mode within the conversation screen.
    /// Only meaningful when `screen == AppScreen::Conversation`.
    pub mode: ConversationMode,
    /// Secondary-panel overlay state and focus.
    pub panel: PanelOverlayState,
}

/// Metadata captured when a response block is opened, stored until the first
/// token arrives and is applied to the output line header.
///
/// `ts` is the wall-clock timestamp at submission time.
/// `model` is the model display string at submission time; empty when no model
/// is known (e.g., auto selection or session-restored messages).
/// Consumers: `AgentStatus`, `append_to_last_line`, `push_error_line`.
#[derive(Clone, bon::Builder)]
pub struct PendingResponseMeta {
    /// Wall-clock timestamp captured at submit time.
    pub ts: TimestampMs,
    /// Model display label at submit time, or empty string if unknown.
    pub model: ModelLabel,
}

/// Header metadata for the first line of a message block.
///
/// `timestamp` is the dimmed `[HH:MM:SS]` prefix shown on the first line of
/// every message block. `model_prefix` is the model label shown for agent
/// responses only: `"claude-sonnet-4.6"` renders as `"claude-sonnet-4.6 > "`
/// before the content span.
/// Consumers: `OutputLine`, `output_line_to_ratatui`, `rendered_line_text`.
#[derive(Default, Clone)]
pub struct LineHeader {
    /// Wall-clock timestamp for the first line of a message block, or `None`
    /// for continuation lines.
    pub timestamp: Option<TimestampMs>,
    /// Model name for agent response lines, or `None` for user input,
    /// system messages, tool-call lines, and continuation lines.
    pub model_prefix: Option<ModelLabel>,
}

/// Metadata for tool-call output lines, preserving structured info for rendering.
///
/// Stores the tool name and arguments as structured data alongside the formatted
/// output line text. This allows rendering logic to access tool metadata without
/// string parsing. Metadata is optional and only populated for `LineKind::ToolCall`
/// lines.
///
/// Invariants:
/// - `tool_name` and `tool_args` are set once at `OutputLine` creation and never mutated
/// - Only present on lines with `kind == LineKind::ToolCall`
/// - Safe to ignore; render logic defaults to text-only display when metadata is None
#[derive(Clone, Debug)]
pub struct LineMetadata {
    /// The name of the tool that was called (e.g., "view", "grep", "shell_exec").
    pub tool_name: augur_domain::domain::string_newtypes::ToolName,
    /// The arguments passed to the tool (full JSON structure).
    pub tool_args: serde_json::Value,
}

/// The rendering style for a single output line.
///
/// Variants are mutually exclusive; exactly one applies per line. The
/// renderer (`output_line_to_ratatui`) and the `append_to_last_line`
/// logic in `AppState` both branch on this enum.
/// Consumers: `output_line_to_ratatui`, `append_to_last_line`, `push_tool_call_line`,
/// `push_self_feedback_line`, `select_user_input_lines`.
#[derive(Default, Clone, Debug, PartialEq)]
pub enum LineKind {
    /// Normal agent text, system messages, and blank separators.
    #[default]
    Plain,
    /// System messages that should preserve their transcript identity.
    ///
    /// Rendered like plain text, but kept distinct so the renderer can treat
    /// them as a visible transcript boundary and preserve them across scroll
    /// recalculation.
    System,
    /// User-submitted message; rendered with dark green background.
    UserInput,
    /// Tool-call header or progress entry; rendered with `Modifier::DIM` styling.
    ///
    /// `append_to_last_line` treats these as append barriers - it will not
    /// append to a `ToolCall` line, inserting a blank separator instead.
    ToolCall,
    /// Error message; rendered with red+bold styling.
    ///
    /// `append_to_last_line` treats error lines as append barriers.
    Error,
    /// Sub-agent self-feedback line (from `ToolPartialResult` events).
    ///
    /// Rendered with `Modifier::DIM | Modifier::ITALIC` so the agent's
    /// internal monologue is visually distinct from both normal output and
    /// tool-call headers. `append_to_last_line` treats these as append barriers.
    SelfFeedback,
}

#[derive(Clone, bon::Builder)]
/// A single line in the output pane, carrying text and rendering hints.
///
/// `kind` signals the renderer which visual style to apply: `UserInput` uses
/// a dark green background, `ToolCall` uses dimmed styling, `Error` uses
/// red+bold, and `SelfFeedback` uses dim+italic for sub-agent monologue.
/// `header` carries the timestamp and optional model prefix for the first
/// line of each message block.
pub struct OutputLine {
    /// The text content of this display line.
    pub text: OutputText,
    /// The rendering style and append-barrier role for this line.
    pub kind: LineKind,
    /// Header metadata for the first line of a message block.
    ///
    /// `None`-timestamp lines are continuation lines or blank separators. Set by
    /// `push_user_input_line` for user messages and by `append_to_last_line`
    /// when `AgentStatus::pending_response` is armed. `model_prefix` is set
    /// only for agent response lines when a model was active at submit time.
    pub header: LineHeader,
    /// Optional metadata for tool-call lines.
    ///
    /// When present, contains the original tool name and arguments preserved
    /// from the `ToolCallStarted` event. Used for render-time access to
    /// structured tool information without string parsing. Only populated for
    /// `LineKind::ToolCall` lines; ignored for other line kinds.
    pub metadata: Option<LineMetadata>,
}

impl OutputLine {
    /// Create a plain output line with no special styling or timestamp.
    pub fn plain(text: impl Into<OutputText>) -> Self {
        OutputLine::builder()
            .text(text.into())
            .kind(LineKind::Plain)
            .header(LineHeader::default())
            .build()
    }

    /// Create a user-input output line with no timestamp (timestamp is added separately).
    pub fn user_input(text: impl Into<OutputText>) -> Self {
        OutputLine::builder()
            .text(text.into())
            .kind(LineKind::UserInput)
            .header(LineHeader::default())
            .build()
    }

    /// Create a tool-call output line with dimmed styling and no timestamp.
    ///
    /// Tool-call lines are rendered with `Modifier::DIM` styling. They act as visual
    /// separators - `append_to_last_line` will not append to a tool-call line; it
    /// creates a new plain line instead. Used by `push_tool_call_line`.
    pub fn tool_call(text: impl Into<OutputText>) -> Self {
        OutputLine::builder()
            .text(text.into())
            .kind(LineKind::ToolCall)
            .header(LineHeader::default())
            .build()
    }

    /// Create a tool-call output line with metadata preservation.
    ///
    /// This factory stores the tool name and arguments as structured metadata
    /// alongside the formatted text. Use this when creating tool-call lines
    /// from `ToolCallStarted` events to preserve information for render-time use.
    ///
    /// Parameters:
    /// - `text`: The formatted tool-call summary string
    /// - `tool_name`: The name of the tool being called
    /// - `tool_args`: The JSON arguments passed to the tool
    ///
    /// Returns: A new `OutputLine` with kind `ToolCall` and populated metadata.
    pub fn tool_call_with_metadata(
        text: impl Into<OutputText>,
        tool_name: augur_domain::domain::string_newtypes::ToolName,
        tool_args: serde_json::Value,
    ) -> Self {
        OutputLine::builder()
            .text(text.into())
            .kind(LineKind::ToolCall)
            .header(LineHeader::default())
            .metadata(LineMetadata {
                tool_name,
                tool_args,
            })
            .build()
    }

    /// Create an error output line with red+bold styling and no timestamp.
    ///
    /// Error lines are rendered with red foreground and bold styling. They act as
    /// visual separators - `append_to_last_line` will not append to an error line.
    /// Used by `push_error_line` in `AppState`.
    pub fn error(text: impl Into<OutputText>) -> Self {
        OutputLine::builder()
            .text(text.into())
            .kind(LineKind::Error)
            .header(LineHeader::default())
            .build()
    }

    /// Create a self-feedback output line with dim+italic styling and no timestamp.
    ///
    /// Self-feedback lines carry sub-agent monologue from `ToolPartialResult` events.
    /// They act as append barriers - `append_to_last_line` will not append to them.
    /// Used by `push_self_feedback_line` in `AppState`.
    pub fn self_feedback(text: impl Into<OutputText>) -> Self {
        OutputLine::builder()
            .text(text.into())
            .kind(LineKind::SelfFeedback)
            .header(LineHeader::default())
            .build()
    }
}

/// A raw screen coordinate used to mark a selection endpoint.
///
/// Stores column and row as reported by crossterm mouse events. Used as
/// components of `OutputSelection` to define the selected text region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionPoint {
    /// Terminal column (0-indexed from left).
    pub col: u16,
    /// Terminal row (0-indexed from top).
    pub row: u16,
}

/// An active text selection in the output pane, defined by two screen positions.
///
/// `anchor` is the position where the mouse was first pressed. `cursor` is the
/// current drag position. Either may be the logical start or end - callers must
/// normalize before use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputSelection {
    /// Fixed endpoint set when the mouse button was pressed.
    pub anchor: SelectionPoint,
    /// Moving endpoint updated as the mouse drags.
    pub cursor: SelectionPoint,
}

/// Interior-mutable rectangles for rendering and mouse event handling boundaries.
#[derive(Clone)]
pub struct PanelAreas {
    /// Bounding rectangle of the output zone as recorded by the last render call.
    ///
    /// Updated each frame by `render_output` via interior mutability. Read by the
    /// mouse event handler to restrict wheel scrolling to the output zone only.
    /// Defaults to `Rect::default()` (zero area) until the first render.
    pub output_area: Cell<Rect>,
    /// Bounding rectangle of the plan panel as recorded by the last render call.
    ///
    /// Updated each frame by `render_plan_layout` and `render_guided_plan_layout`
    /// via interior mutability. Read by `handle_plan_mouse_scroll` to route scroll
    /// events to the plan panel only when the pointer is within its bounds.
    /// Defaults to `Rect::default()` (zero area) until the first render in plan mode.
    pub plan_panel_area: Cell<Rect>,
    /// Bounding rectangle of the secondary (agent feed) panel as recorded by the last render call.
    ///
    /// Updated each frame by `render_secondary_container` via interior mutability.
    /// Read by mouse event handlers to route scroll events to the agent feed panel
    /// only when the pointer is within its bounds. Defaults to `Rect::default()` (zero area)
    /// until the first render of the secondary panel.
    pub secondary_panel_area: Cell<Rect>,
}

impl Default for PanelAreas {
    fn default() -> Self {
        Self {
            output_area: Cell::new(Rect::default()),
            plan_panel_area: Cell::new(Rect::default()),
            secondary_panel_area: Cell::new(Rect::default()),
        }
    }
}

#[derive(Clone, bon::Builder)]
/// All lines accumulated in the output pane.
pub struct OutputPane {
    /// Accumulated output lines. Each element is one display line.
    pub lines: Vec<OutputLine>,
    /// Number of lines scrolled up from the bottom. 0 means follow the latest output.
    ///
    /// Interior-mutable so the render path can recalculate the offset on width change
    /// without requiring `&mut AppState` through the entire render call chain.
    #[builder(default)]
    pub scroll_offset: Cell<ScrollOffset>,
    /// Last content-area width (in columns) used during render.
    ///
    /// Interior-mutable sentinel updated by `render_output` on every frame.
    /// A value of 0 means "not yet rendered"; the first real render sets it.
    /// When the value changes between frames, `scroll_offset` is recalculated
    /// to preserve the user's visual position after text reflows.
    #[builder(default)]
    pub last_render_width: Cell<usize>,
    /// Interior-mutable rendering area boundaries for mouse event routing.
    pub panel_areas: PanelAreas,
    /// Active text selection, or `None` when nothing is selected.
    ///
    /// Set by `SelectionStart` (mouse down), updated by `SelectionExtend` (drag),
    /// and cleared by `ClearSelection` (click outside) or after clipboard copy.
    pub selection: Option<OutputSelection>,
}

/// Completion state for the `/model` picker: list of available models and navigation index.
///
/// Populated from `PromptPane::available_models` each time the user types `/model`.
/// Cleared when a model is selected, Esc is pressed, or the buffer no longer starts
/// with `/model`.
#[derive(Default, Clone, bon::Builder)]
pub struct ModelCompletion {
    /// Models matching the current `/model` buffer prefix.
    pub items: Vec<ModelOption>,
    /// Index of the currently highlighted model, or `None`.
    pub selected: Option<usize>,
    /// Thinking mode picker state shown after a model is confirmed.
    #[builder(default)]
    pub thinking_mode: ThinkingModeCompletion,
}

/// State for the two-step thinking mode selection overlay.
///
/// When the user confirms a model with Enter, `pending_model_id` is set and the
/// model list is cleared. A second overlay shows the five `ReasoningEffort` options.
/// When the user confirms an effort level (or presses Enter without selecting one),
/// `handle_thinking_mode_confirm` reads this struct, calls `set_model_with_options`,
/// and clears both `pending_model_id` and `selected`.
#[derive(Default, Clone)]
pub struct ThinkingModeCompletion {
    /// Model id waiting for a thinking mode choice. `None` when the picker is closed.
    pub pending_model_id: Option<ModelId>,
    /// Index into `ReasoningEffort::options()` for the highlighted row, or `None`.
    pub selected: Option<usize>,
}

impl ModelCompletion {
    /// Open the thinking mode picker for `model_id`.
    ///
    /// Clears the model list and selection, then arms `thinking_mode` with the
    /// chosen model id so the second-step overlay can confirm a `ReasoningEffort`.
    pub fn open_thinking_mode(&mut self, model_id: ModelId) {
        self.items.clear();
        self.selected = None;
        self.thinking_mode.pending_model_id = Some(model_id);
        self.thinking_mode.selected = None;
    }
}

/// Completion state for the prompt pane: command hints, file hints, and model picker.
///
/// Extracted from `PromptPane` to accommodate both command and file completion
/// lists without exceeding the 5-field struct limit. Command and file completions
/// are mutually exclusive at runtime: only one list is populated at a time.
/// Model completions are active when the buffer starts with `/model`.
#[derive(Default, Clone, bon::Builder)]
pub struct PromptCompletions {
    /// Slash-command completions matching the current `/`-prefix in the buffer.
    pub commands: Vec<CommandDef>,
    /// Index of the currently highlighted command completion, or `None`.
    pub command_selected: Option<usize>,
    /// File path completions matching the current `@`-prefix token in the buffer.
    pub files: Vec<FileCompletion>,
    /// Index of the currently highlighted file completion, or `None`.
    pub file_selected: Option<usize>,
    /// Model picker completions active when the buffer starts with `/model`.
    pub model_picker: ModelCompletion,
}

impl PromptCompletions {
    /// Return `true` when no command, file, or model completions are available,
    /// and no thinking mode picker is open.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> IsPredicate {
        IsPredicate::from(
            self.commands.is_empty()
                && self.files.is_empty()
                && self.model_picker.items.is_empty()
                && self.model_picker.thinking_mode.pending_model_id.is_none(),
        )
    }
}

/// History navigation state for the prompt pane.
///
/// Groups the cursor position and saved draft text so `PromptPane` stays
/// within the 5-field limit.
#[derive(Default, Clone)]
pub struct HistoryNav {
    /// Index from the end of submitted history. `None` = at the live entry.
    pub pos: Option<usize>,
    /// Buffer text saved when history navigation first started.
    /// Restored when Down key moves past the most recent entry.
    pub draft: Option<String>,
}

/// Available models and the currently active model id for the model picker overlay.
///
/// Groups the full available-model list and the active model id so both can be
/// accessed and updated together. Consumed by `refresh_model_hints` in
/// `key_dispatch.rs` and by `input.rs` model-event handlers.
#[derive(Default, Clone)]
pub struct ModelPickerData {
    /// Active endpoint model options shown by `/model`.
    ///
    /// For provider endpoints, this is sourced from endpoint catalogs loaded from
    /// provider YAML files. `AgentOutput::ModelsAvailable` may update this only on
    /// auto-capable endpoints (for example, Copilot).
    pub available: Vec<ModelOption>,
    /// Id of the currently active model, updated by `AgentOutput::ActiveModelChanged`
    /// and `AgentOutput::UsageUpdate`. `None` before the session's first model report.
    pub active_id: Option<ModelId>,
    /// Per-endpoint model catalogs used to refresh model choices on `/switch`.
    ///
    /// Built at startup from `AppConfig.endpoints` and used by submit handling to
    /// replace stale provider model lists when the endpoint changes.
    #[allow(clippy::struct_excessive_bools)]
    pub endpoint_catalog: Vec<EndpointModelCatalog>,
}

/// Model-catalog metadata for a single endpoint - re-exported from core domain.
pub use augur_domain::domain::EndpointModelCatalog;

#[derive(Clone, bon::Builder)]
/// Mutable state for the bottom prompt input pane.
pub struct PromptPane {
    /// Current user-typed text not yet submitted.
    pub buffer: PromptBuffer,
    /// Byte offset of the cursor within `buffer`.
    pub cursor: usize,
    /// Active completion lists: command hints (buffer starts with `/`) or file
    /// hints (buffer contains `@`). Both lists are empty when neither applies.
    pub completions: PromptCompletions,
    /// History navigation state: cursor position and saved draft text.
    ///
    /// `pos` is the offset from the end of user-input lines (most-recent-first).
    /// `None` = at the live entry. `Some(0)` = most recently submitted line.
    /// `draft` holds the in-progress buffer text saved when navigation first starts.
    /// Reset to default on char input or paste.
    #[builder(default)]
    pub history: HistoryNav,
    /// Available models and active model id for the model picker overlay.
    /// Populated from endpoint-catalog startup data and `ActiveModelChanged`.
    pub models: ModelPickerData,
}

#[derive(Clone, bon::Builder)]
/// Thinking indicator sub-state: spinner visibility, label, and animation tick.
///
/// Grouped from `AgentStatus` to free field slots for `pending_tool_call_line_idx`.
/// The three fields share a lifecycle - all reset together at turn start and when
/// `is_active` is cleared.
pub struct ThinkingIndicator {
    /// True while the agent is processing a turn; drives the status indicator.
    pub is_active: IsActive,
    /// Text label shown in the thinking row when `is_active` is true.
    ///
    /// Updated to `"Calling <name>..."` on `ToolCallStarted` events and reset
    /// to "Thinking..." on Token events and at turn start.
    pub label: StatusLabel,
    /// Rotating Braille spinner frame index (0-9). Incremented every 100 ms
    /// by the TUI actor's ticker while `is_active` is true.
    pub spinner_tick: u8,
}

impl Default for ThinkingIndicator {
    fn default() -> Self {
        ThinkingIndicator::builder()
            .is_active(IsActive::no())
            .label(StatusLabel::new("Thinking..."))
            .spinner_tick(0)
            .build()
    }
}

#[derive(Clone, bon::Builder)]
/// Agent execution status: endpoint selection, thinking indicator, and response metadata.
///
/// Extracted from `AppState` to free a field slot and keep the struct at the
/// 5-field limit. Updated each frame from the session watch channel.
pub struct AgentStatus {
    /// Currently active endpoint name displayed in the status bar.
    pub endpoint_name: EndpointName,
    /// Thinking spinner state: active flag, label text, and animation tick.
    pub thinking: ThinkingIndicator,
    /// Metadata to stamp on the first line of the next response block.
    ///
    /// Set by `handle_submit` (live turns) and `hydrate_output_from_messages`
    /// (history replay) before the first token of each response arrives.
    /// Consumed and cleared by `append_to_last_line` the first time it fires.
    pub pending_response: Option<PendingResponseMeta>,
    /// Output line index of the pending `ToolSummary` placeholder pushed by
    /// `push_pending_tool_summary`. Filled or cleared when `ToolCallCompleted`
    /// arrives. `None` when no tool is currently in-flight.
    pub pending_tool_call_line_idx: Option<Count>,
    /// Idempotency guard for `finish_turn_output`.
    ///
    /// Set to `true` by `finish_turn_output` on the first call so that a second
    /// call (e.g. both `Done` and `TurnComplete` fire for the same turn) is a
    /// no-op and does not append duplicate blank lines.  Reset to `false` by
    /// `push_user_input_line` when the next user turn begins.
    #[builder(default)]
    pub is_turn_complete: IsTurnComplete,
}

/// Context window state: backoff state for the status bar countdown.
///
/// Grouped here so `StatusBarData` stays within the 5-field limit.
/// `backoff_until` is set when a "requests exceeded" exponential backoff begins;
/// the renderer shows a countdown in the status bar while it is `Some`.
#[derive(Default, Clone, bon::Builder)]
pub struct ContextWindowState {
    /// Deadline instant for the current exponential backoff wait.
    ///
    /// `Some` while the LLM provider is sleeping after a "requests exceeded" 429.
    /// Set when `AgentOutput::BackoffStarted` arrives; cleared on `Done`, `Error`,
    /// or `Interrupted`. The status bar reads this to compute and display the
    /// remaining wait as `| [Backoff: Xs]`.
    pub backoff_until: Option<Instant>,
}

impl ContextWindowState {
    /// Reset backoff state for a new session.
    ///
    /// Consumers: `session_restore::apply_restored_session`.
    pub fn reset_for_new_session(&mut self) {
        self.backoff_until = None;
    }
}

/// Accumulated usage data for the status bar: token totals and latest context snapshot.
///
/// Decomposed from `StatusBarData` to keep field count within the 5-field limit.
/// `StatusBarData` implements `Deref<Target = StatusBarUsage>` so callers can access
/// `status.token_totals` and `status.last_context` directly via auto-deref.
#[derive(Default, Clone, bon::Builder)]
pub struct StatusBarUsage {
    /// Accumulated token and cost totals; updated via `UsageSnapshot` TUI events.
    pub token_totals: ProjectTokenTotals,
    /// Most-recent context window snapshot; `None` until the first update arrives.
    pub last_context: Option<ContextUsageStats>,
    /// Absolute tracker snapshot at the last `/new-session` reset boundary.
    ///
    /// Snapshot ticks display `current - baseline`, so session-local totals reset
    /// to zero without mutating historical tracker state.
    #[builder(default)]
    pub token_totals_baseline: ProjectTokenTotals,
    /// Marker set by `/new-session`; the next snapshot captures a new baseline.
    #[builder(default)]
    pub reset_usage_on_next_snapshot: ShouldResetUsage,
}

/// Status bar display data updated after each completed agent turn.
///
/// Holds the cwd, git branch, formatted model label, context window usage, and
/// accumulated token totals. All base fields are refreshed at startup and after
/// each `AgentOutput::Done`. `token_totals` is updated by `UsageSnapshot` events.
/// Implements `Deref<Target = StatusBarUsage>` for direct field access.
#[derive(Default, Clone, bon::Builder)]
pub struct StatusBarData {
    /// Formatted model + effort label, e.g. `"claude-sonnet-4-6 (high)"`.
    pub model_display: ModelLabel,
    /// Current git branch name, or `None` when not inside a git repository.
    pub git_branch: Option<GitBranch>,
    /// Current working directory as a display string.
    pub cwd: WorkingDir,
    /// Context window usage and auto-compact state.
    pub context_window: ContextWindowState,
    /// Accumulated usage data: token totals and latest context snapshot.
    #[builder(default)]
    pub usage: StatusBarUsage,
}

impl std::ops::Deref for StatusBarData {
    type Target = StatusBarUsage;

    fn deref(&self) -> &StatusBarUsage {
        &self.usage
    }
}

impl std::ops::DerefMut for StatusBarData {
    fn deref_mut(&mut self) -> &mut StatusBarUsage {
        &mut self.usage
    }
}

#[derive(bon::Builder)]
/// Top-level UI state owned exclusively by the TuiActor.
///
/// All fields are plain owned data - no channels, no shared references.
/// Decomposed into sub-structs to keep field count at 5.
pub struct AppState {
    /// Output pane state: accumulated lines and scroll position.
    pub output: OutputPane,
    /// Prompt pane state: input buffer and cursor.
    pub prompt: PromptPane,
    /// Agent execution status: endpoint name and thinking indicator.
    pub agent: AgentStatus,
    /// Status bar display data: tokens, model label, cwd, git branch.
    pub status: StatusBarData,
    /// Current display mode, ask panel overlay, and input focus state.
    pub interaction: AppInteraction,
}
