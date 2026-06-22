//! Key event classification and application to AppState. Pure functions; no I/O.

#[path = "tui_input/agent_output.rs"]
mod agent_output;
#[path = "tui_input/classify.rs"]
mod classify;
#[path = "tui_input/panel_output.rs"]
mod panel_output;
#[path = "tui_input/prompt_completion.rs"]
mod prompt_completion;
#[path = "tui_input/prompt_edit.rs"]
mod prompt_edit;
#[path = "tui_input/query.rs"]
mod query;

use crate::domain::tui_state::{
    AppState, LineKind, PendingResponseMeta, PickerState, QueryState, current_timestamp_ms,
};
use crate::domain::tui_status::refresh_status_bar_base_fields;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype};
use augur_domain::domain::types::{AgentFeedOutput, AgentOutput};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use std::time::Instant;

/// Number of lines scrolled per mouse-wheel tick over the output pane.
///
/// Applied to both `ScrollUp` and `ScrollDown` variants produced by
/// `classify_mouse`. Three lines per tick matches common terminal scrolling
/// behavior and keeps the visual motion proportional to the scroll gesture.
pub const MOUSE_SCROLL_LINES: usize = 3;
/// Number of lines scrolled by PageUp/PageDown.
pub(crate) const KEY_SCROLL_LINES_PAGE: usize = 10;
/// Number of lines scrolled by Ctrl+U/Ctrl+D.
pub(crate) const KEY_SCROLL_LINES_HALF: usize = 5;

pub use agent_output::apply_agent_output;
pub(crate) use agent_output::push_turn_end;
pub use classify::{classify_key, classify_mouse, classify_picker_key, classify_query_key};
pub use panel_output::apply_agent_feed_output;
pub use panel_output::apply_ask_output;
pub(crate) use prompt_completion::apply_file_completion;
pub(crate) use prompt_completion::apply_tab_completion;
pub use prompt_edit::{apply_key, apply_picker_key, insert_paste};
pub use query::apply_query_key;

/// Classified action resulting from a mouse event.
#[derive(Clone, Debug, PartialEq)]
pub enum MouseAction {
    /// Scroll the output pane up (toward older content) by N lines.
    ScrollUp(usize),
    /// Scroll the output pane down (toward newer content) by N lines.
    ScrollDown(usize),
    /// Right mouse button pressed: paste clipboard content into the prompt.
    RightClick,
    /// Left mouse button pressed inside the output pane: begin a new selection.
    ///
    /// Both `row` and `col` are terminal screen coordinates from the mouse event.
    SelectionStart { row: u16, col: u16 },
    /// Left mouse button dragged inside the output pane: extend the active selection.
    ///
    /// Both `row` and `col` are terminal screen coordinates from the mouse event.
    SelectionExtend { row: u16, col: u16 },
    /// Left mouse button pressed outside the output pane: clear any active selection.
    ClearSelection,
    /// Any mouse event that does not affect TUI state.
    Ignored,
}

/// Classified action resulting from a single key event in chat mode.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyAction {
    /// Enter key: submit the current prompt buffer.
    Submit,
    /// A printable character to insert at the current cursor position.
    AppendChar(char),
    /// Backspace: delete the character immediately before the cursor.
    Backspace,
    /// Delete: delete the character immediately after (at) the cursor.
    Delete,
    /// Scroll the output pane up by N lines.
    ScrollUp(usize),
    /// Scroll the output pane down by N lines.
    ScrollDown(usize),
    /// Left arrow: move cursor one character toward the start.
    CursorLeft,
    /// Right arrow: move cursor one character toward the end.
    CursorRight,
    /// Home key: move cursor to byte position 0.
    CursorHome,
    /// End key: move cursor to the end of the buffer.
    CursorEnd,
    /// Tab key: complete the currently selected (or first) command in the completion list.
    ///
    /// Copies the command's usage text into the prompt buffer and clears the
    /// completion list, leaving the cursor at the end of the completed text.
    /// A no-op when no completions are visible.
    Tab,
    /// Up arrow: context-sensitive navigation.
    ///
    /// When completions are visible: moves `completion_selected` one step toward the
    /// start of the list (wraps: `Some(0) → None → Some(last)`).
    /// When completions are absent and the buffer is empty, or when already navigating
    /// history: navigates the input history toward older entries.
    CompletionUp,
    /// Down arrow: context-sensitive navigation.
    ///
    /// When completions are visible: moves `completion_selected` one step toward the
    /// end of the list (wraps: `Some(last) → None → Some(0)`).
    /// When already navigating history: navigates toward newer entries. Reaching the
    /// live entry (past the most recent) restores an empty buffer.
    CompletionDown,
    /// Ctrl+C or `/quit` command: exit the TUI.
    Quit,
    /// Esc key: cancel the currently running agent turn, if any.
    ///
    /// When the agent is thinking (`is_thinking == true`), this signals
    /// `handle_cancel_or_submit` in the TUI actor to call `interrupt()` on the
    /// agent handle and push `[stopped]` to the output pane. When the agent
    /// is idle, this is a no-op. Handled at the dispatch layer; `apply_key` is
    /// a pure no-op for this variant.
    CancelThinking,
    /// Ctrl+V: request a paste from the OS clipboard.
    ///
    /// This variant is a signal; `apply_key` is a no-op for it. The TUI actor
    /// reads the clipboard and calls `apply_key(state, KeyAction::Paste(text))`
    /// when the clipboard read succeeds.
    RequestPaste,
    /// Insert a string at the current cursor position.
    ///
    /// Text is normalized before insertion: `\r\n` and lone `\r`/`\n` are
    /// replaced with a single space so the single-line prompt buffer stays
    /// free of embedded newlines. Produced by bracketed-paste terminal events
    /// and by `RequestPaste` clipboard reads.
    Paste(String),
    /// Tab key: toggles ask panel focus between `Main` and `Ask`.
    ///
    /// When `ask_panel` is `None`, this is a no-op. Produced by `KeyCode::Tab`
    /// and handled at the dispatch layer by `dispatch_chat_key`.
    ToggleAskFocus,
    /// Shift+Tab: open the ask panel when closed.
    ///
    /// When `ask_panel` is already `Some`, this is a no-op. Produced by
    /// `KeyCode::BackTab` (crossterm's Shift+Tab encoding) unconditionally.
    ShiftTab,
    /// Ctrl+T: toggle the agent feed secondary panel.
    ///
    /// Opens the agent feed when `secondary_view` is `None`, closes when
    /// `secondary_view` is `Some(AgentFeed)`, and switches from Ask to AgentFeed
    /// when `secondary_view` is `Some(Ask)`. Handled by `dispatch_chat_key`.
    ToggleAgentFeed,
    /// Ctrl+, : select the previous tracked agent feed.
    AgentFeedPrev,
    /// Ctrl+. : select the next tracked agent feed.
    AgentFeedNext,
    /// Ctrl+W: close the currently-open secondary panel.
    ///
    /// Sets `secondary_view` to `None` regardless of which panel is currently
    /// open. Handled by `dispatch_chat_key`; `apply_key` is a no-op for this
    /// variant.
    CloseSecondaryPanel,
    /// Any unhandled key; produces no state change.
    Ignored,
}

/// Classified action resulting from a single key event in session picker mode.
#[derive(Clone, Debug, PartialEq)]
pub enum PickerKeyAction {
    /// Up arrow or Ctrl+Up: move selection to the previous session.
    SelectUp,
    /// Down arrow or Ctrl+Down: move selection to the next session.
    SelectDown,
    /// Enter: restore the currently selected session.
    Confirm,
    /// `d` or `D`: delete the currently selected saved session.
    Delete,
    /// `n` or `N`: discard the picker and start a new session.
    NewSession,
    /// Ctrl+C: exit the TUI without restoring.
    Quit,
    /// Any other key; the picker state is unchanged.
    Ignored,
}

/// Classified action resulting from a single key event in query overlay mode.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryKeyAction {
    /// Up arrow: move selection to the previous choice (wraps from first to last).
    SelectUp,
    /// Down arrow: move selection to the next choice (wraps from last to first).
    SelectDown,
    /// A printable character to append to the free-form input buffer.
    AppendFreeform(char),
    /// Backspace: remove the last character from the free-form buffer.
    Backspace,
    /// Enter: submit the current selection or free-form input.
    Submit,
    /// Ctrl+C: cancel the query and exit the TUI.
    Quit,
    /// Any other key; produces no state change.
    Ignored,
}
