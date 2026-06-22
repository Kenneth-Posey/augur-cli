use augur_core::actors::agent::agent_ops::AgentOutput;
use augur_domain::domain::newtypes::IsThinking;
use augur_tui::domain::newtypes::{Count, NumericNewtype};
use augur_tui::domain::string_newtypes::{
    ChoiceText, EndpointName, ModelLabel, OutputText, PromptText, StringNewtype, ToolName,
};
use augur_tui::domain::tui_input::{
    KeyAction, MOUSE_SCROLL_LINES, MouseAction, QueryKeyAction, apply_agent_feed_output,
    apply_agent_output, apply_ask_output, apply_key, apply_query_key, classify_key, classify_mouse,
    classify_query_key,
};
use augur_tui::domain::tui_state::{AppScreen, AppState, LineKind, QueryState};
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::layout::Rect;
use std::ops::ControlFlow;

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: mods,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn default_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

fn completions_are_empty(completions: &augur_tui::domain::tui_state::PromptCompletions) -> bool {
    completions.commands.is_empty()
        && completions.files.is_empty()
        && completions.model_picker.items.is_empty()
}

/// Verifies that pressing Enter produces KeyAction::Submit.
#[test]
fn classify_enter_is_submit() {
    let action = classify_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(action, KeyAction::Submit));
}

/// Verifies that Ctrl+C produces KeyAction::Quit.
#[test]
fn classify_ctrl_c_is_quit() {
    let action = classify_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(matches!(action, KeyAction::Quit));
}

/// Verifies that a printable character with no modifiers produces KeyAction::AppendChar.
#[test]
fn classify_char_is_append() {
    let action = classify_key(key(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(matches!(action, KeyAction::AppendChar('x')));
}

/// Verifies that Backspace produces KeyAction::Backspace.
#[test]
fn classify_backspace_is_backspace() {
    let action = classify_key(key(KeyCode::Backspace, KeyModifiers::NONE));
    assert!(matches!(action, KeyAction::Backspace));
}

/// Verifies that Page Up produces KeyAction::ScrollUp with 10 lines.
#[test]
fn classify_page_up_is_scroll_up_10() {
    let action = classify_key(key(KeyCode::PageUp, KeyModifiers::NONE));
    assert!(matches!(action, KeyAction::ScrollUp(10)));
}

/// Verifies that apply_key AppendChar adds the character to the prompt buffer at cursor.
#[test]
fn apply_key_append_updates_buffer() {
    let mut state = default_state();
    let quit = apply_key(&mut state, KeyAction::AppendChar('h'));
    assert!(matches!(quit, ControlFlow::Continue(())));
    assert_eq!(state.prompt.buffer, "h".into());
    assert_eq!(state.prompt.cursor, 1);
}

/// Verifies that apply_key Backspace removes the character before the cursor.
///
/// Cursor must be set to end of buffer for backspace to remove the last char.
#[test]
fn apply_key_backspace_removes_char() {
    let mut state = default_state();
    state.prompt.buffer.push_str("ab");
    state.prompt.cursor = 2;
    let quit = apply_key(&mut state, KeyAction::Backspace);
    assert!(matches!(quit, ControlFlow::Continue(())));
    assert_eq!(state.prompt.buffer, "a".into());
    assert_eq!(state.prompt.cursor, 1);
}

/// Verifies that apply_key Quit returns true to signal the TUI should exit.
#[test]
fn apply_key_quit_returns_true() {
    let mut state = default_state();
    let quit = apply_key(&mut state, KeyAction::Quit);
    assert!(matches!(quit, ControlFlow::Break(())));
}

/// Verifies that apply_agent_output Token appends the text to the output and
/// does not clear is_thinking.
#[test]
fn apply_agent_output_token_appends_to_state() {
    let mut state = default_state();
    state.agent.thinking.is_active = true.into();
    apply_agent_output(&mut state, AgentOutput::Token(OutputText::new("hello")));
    assert_eq!(state.output.lines.len(), 1);
    assert_eq!(state.output.lines[0].text.as_str(), "hello");
    // Token alone does not clear is_thinking
    assert!(state.agent.thinking.is_active);
}

/// Verifies that apply_agent_output Done pushes two newlines (blank separator) and clears is_thinking.
#[test]
fn apply_agent_output_done_pushes_newline_and_clears_thinking() {
    let mut state = default_state();
    state.push_output_token(OutputText::new("response"));
    state.agent.thinking.is_active = true.into();
    apply_agent_output(&mut state, AgentOutput::Done);
    assert_eq!(state.output.lines.len(), 3);
    assert!(!state.agent.thinking.is_active);
}

/// Verifies that TurnComplete (the Copilot SDK's session-idle signal) clears is_thinking
/// and pushes two separator newlines - identical behaviour to Done.
/// Regression: before the fix, TurnComplete was a no-op and the spinner never stopped.
#[test]
fn apply_agent_output_turn_complete_clears_thinking_and_pushes_separator() {
    let mut state = default_state();
    state.push_output_token(OutputText::new("response"));
    state.agent.thinking.is_active = true.into();
    apply_agent_output(&mut state, AgentOutput::TurnComplete);
    assert_eq!(
        state.output.lines.len(),
        3,
        "two separator newlines expected after TurnComplete"
    );
    assert!(
        !state.agent.thinking.is_active,
        "is_thinking must be cleared by TurnComplete"
    );
}

/// Verifies that MessageBreak pushes two blank lines (same as turn-end separator) without
/// clearing is_thinking, so successive LLM messages are visually separated in the output pane.
#[test]
fn apply_agent_output_message_break_pushes_blank_lines_without_clearing_thinking() {
    let mut state = default_state();
    state.push_output_token(OutputText::new("first response"));
    state.agent.thinking.is_active = true.into();
    apply_agent_output(&mut state, AgentOutput::MessageBreak);
    // Two newlines appended: one to end current line, one blank separator
    assert_eq!(state.output.lines.len(), 3);
    // is_thinking must remain true - the turn is still in progress
    assert!(state.agent.thinking.is_active);
}
///
/// Buffer "ab", cursor=1 → AppendChar('X') → buffer "aXb", cursor=2.
#[test]
fn append_char_inserts_at_cursor_middle() {
    let mut state = default_state();
    state.prompt.buffer = "ab".into();
    state.prompt.cursor = 1;
    let _ = apply_key(&mut state, KeyAction::AppendChar('X'));
    assert_eq!(state.prompt.buffer, "aXb".into());
    assert_eq!(state.prompt.cursor, 2);
}

/// Verifies that Backspace removes the character immediately before the cursor.
///
/// Buffer "abc", cursor=2 → Backspace → buffer "ac", cursor=1.
#[test]
fn backspace_removes_char_before_cursor() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 2;
    let _ = apply_key(&mut state, KeyAction::Backspace);
    assert_eq!(state.prompt.buffer, "ac".into());
    assert_eq!(state.prompt.cursor, 1);
}

/// Verifies that Delete removes the character immediately after the cursor.
///
/// Buffer "abc", cursor=1 → Delete → buffer "ac", cursor=1 (unchanged).
#[test]
fn delete_removes_char_at_cursor() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 1;
    let _ = apply_key(&mut state, KeyAction::Delete);
    assert_eq!(state.prompt.buffer, "ac".into());
    assert_eq!(state.prompt.cursor, 1, "cursor must not move after Delete");
}

/// Verifies that Delete at end-of-buffer is a no-op.
///
/// Buffer "ab", cursor=2 (end) → Delete → buffer unchanged.
#[test]
fn delete_at_end_of_buffer_is_noop() {
    let mut state = default_state();
    state.prompt.buffer = "ab".into();
    state.prompt.cursor = 2;
    let _ = apply_key(&mut state, KeyAction::Delete);
    assert_eq!(state.prompt.buffer, "ab".into());
    assert_eq!(state.prompt.cursor, 2);
}

/// Verifies that Delete handles a multi-byte UTF-8 character correctly.
///
/// Buffer "aéb", cursor=1 (before 'é') → Delete → buffer "ab", cursor=1.
/// The full 2-byte sequence for 'é' must be removed without corrupting the string.
#[test]
fn delete_handles_multibyte_char() {
    let mut state = default_state();
    state.prompt.buffer = "aéb".into();
    state.prompt.cursor = 1;
    let _ = apply_key(&mut state, KeyAction::Delete);
    assert_eq!(state.prompt.buffer, "ab".into());
    assert_eq!(state.prompt.cursor, 1);
}

/// Verifies that ToolPartialResult events create SelfFeedback lines.
///
/// Lines produced by sub-agent feedback via ToolPartialResult must have
/// LineKind::SelfFeedback so the renderer applies DIM|ITALIC styling.
#[test]
fn apply_agent_output_tool_partial_creates_self_feedback_lines() {
    let mut state = default_state();
    apply_agent_output(
        &mut state,
        AgentOutput::ToolPartialResult {
            tool_call_id: "".into(),
            output: OutputText::new("analysis complete"),
        },
    );
    let feedback_lines: Vec<_> = state
        .output
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::SelfFeedback)
        .collect();
    assert!(
        !feedback_lines.is_empty(),
        "ToolPartialResult must produce SelfFeedback lines"
    );
    assert!(
        feedback_lines
            .iter()
            .any(|l| l.text.as_str() == "analysis complete"),
        "SelfFeedback line must contain the partial result text"
    );
}

/// Verifies that consecutive ToolPartialResult events with blank lines between
/// paragraphs preserve those blank lines (do not join paragraphs together).
///
/// Sub-agent output often contains paragraph structure. Blank lines must be
/// stored as SelfFeedback lines with empty text, not discarded.
#[test]
fn apply_agent_output_tool_partial_preserves_blank_lines() {
    let mut state = default_state();
    apply_agent_output(
        &mut state,
        AgentOutput::ToolPartialResult {
            tool_call_id: "".into(),
            output: OutputText::new("para one"),
        },
    );
    apply_agent_output(
        &mut state,
        AgentOutput::ToolPartialResult {
            tool_call_id: "".into(),
            output: OutputText::new(""),
        },
    );
    apply_agent_output(
        &mut state,
        AgentOutput::ToolPartialResult {
            tool_call_id: "".into(),
            output: OutputText::new("para two"),
        },
    );
    let feedback_lines: Vec<_> = state
        .output
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::SelfFeedback)
        .collect();
    assert!(
        feedback_lines.len() >= 3,
        "blank lines between paragraphs must be preserved as SelfFeedback lines"
    );
    let has_blank = feedback_lines.iter().any(|l| l.text.as_str().is_empty());
    assert!(
        has_blank,
        "a blank SelfFeedback line must exist between paragraphs"
    );
}

/// Verifies that CursorLeft moves the cursor one character to the left.
#[test]
fn cursor_left_moves_one_char() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 3;
    let _ = apply_key(&mut state, KeyAction::CursorLeft);
    assert_eq!(state.prompt.cursor, 2);
}

/// Verifies that CursorRight moves the cursor one character to the right.
#[test]
fn cursor_right_moves_one_char() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 0;
    let _ = apply_key(&mut state, KeyAction::CursorRight);
    assert_eq!(state.prompt.cursor, 1);
}

/// Verifies that CursorHome moves the cursor to byte position 0.
#[test]
fn cursor_home_moves_to_zero() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 3;
    let _ = apply_key(&mut state, KeyAction::CursorHome);
    assert_eq!(state.prompt.cursor, 0);
}

/// Verifies that CursorEnd moves the cursor to the end of the buffer.
#[test]
fn cursor_end_moves_to_end() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 0;
    let _ = apply_key(&mut state, KeyAction::CursorEnd);
    assert_eq!(state.prompt.cursor, 3);
}

/// Verifies that CursorLeft at position 0 stays at 0 (no underflow).
#[test]
fn cursor_left_at_zero_stays_at_zero() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 0;
    let _ = apply_key(&mut state, KeyAction::CursorLeft);
    assert_eq!(state.prompt.cursor, 0);
}

/// Verifies that CursorRight at the end of the buffer stays at the end.
#[test]
fn cursor_right_at_end_stays_at_end() {
    let mut state = default_state();
    state.prompt.buffer = "abc".into();
    state.prompt.cursor = 3;
    let _ = apply_key(&mut state, KeyAction::CursorRight);
    assert_eq!(state.prompt.cursor, 3);
}

fn make_query_state() -> QueryState {
    let (reply_tx, _reply_rx) = tokio::sync::oneshot::channel::<OutputText>();
    QueryState {
        question: PromptText::new("Choose?"),
        choices: vec![
            ChoiceText::new("yes"),
            ChoiceText::new("no"),
            ChoiceText::new("maybe"),
        ],
        selected: None,
        freeform: PromptText::new(""),
        reply_tx,
    }
}

/// Verifies that the Up arrow key classifies as QueryKeyAction::SelectUp.
#[test]
fn classify_query_key_up_is_select_up() {
    let action = classify_query_key(key(KeyCode::Up, KeyModifiers::NONE));
    assert!(matches!(action, QueryKeyAction::SelectUp));
}

/// Verifies that the Down arrow key classifies as QueryKeyAction::SelectDown.
#[test]
fn classify_query_key_down_is_select_down() {
    let action = classify_query_key(key(KeyCode::Down, KeyModifiers::NONE));
    assert!(matches!(action, QueryKeyAction::SelectDown));
}

/// Verifies that Enter classifies as QueryKeyAction::Submit.
#[test]
fn classify_query_key_enter_is_submit() {
    let action = classify_query_key(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(action, QueryKeyAction::Submit));
}

/// Verifies that Ctrl+C classifies as QueryKeyAction::Quit.
#[test]
fn classify_query_key_ctrl_c_is_quit() {
    let action = classify_query_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(matches!(action, QueryKeyAction::Quit));
}

/// Verifies that a printable character classifies as QueryKeyAction::AppendFreeform.
#[test]
fn classify_query_key_char_is_append_freeform() {
    let action = classify_query_key(key(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(matches!(action, QueryKeyAction::AppendFreeform('x')));
}

/// Verifies that Backspace classifies as QueryKeyAction::Backspace.
#[test]
fn classify_query_key_backspace_is_backspace() {
    let action = classify_query_key(key(KeyCode::Backspace, KeyModifiers::NONE));
    assert!(matches!(action, QueryKeyAction::Backspace));
}

/// Verifies that SelectDown from None selects the first choice (index 0).
///
/// When no choice is selected and the user presses Down, the first choice
/// should become selected. Subsequent Down presses advance to index 1, 2, etc.
#[test]
fn apply_query_key_select_down_sets_first_when_none() {
    let mut qs = make_query_state();
    assert_eq!(qs.selected, None);
    apply_query_key(&mut qs, &QueryKeyAction::SelectDown);
    assert_eq!(qs.selected, Some(Count::new(0).inner()));
    apply_query_key(&mut qs, &QueryKeyAction::SelectDown);
    assert_eq!(qs.selected, Some(Count::new(1).inner()));
}

/// Verifies that SelectUp from the first choice (index 0) wraps to the last choice.
///
/// The up-arrow should wrap around from index 0 to the last index in the list.
#[test]
fn apply_query_key_select_up_wraps_to_last() {
    let mut qs = make_query_state();
    qs.selected = Some(Count::new(0).inner());
    apply_query_key(&mut qs, &QueryKeyAction::SelectUp);
    assert_eq!(
        qs.selected,
        Some(Count::new(2).inner()),
        "should wrap to last index (2)"
    );
}

/// Verifies that AppendFreeform adds the character to freeform and clears selected.
///
/// When the user types a character, the selection is cleared (freeform takes priority)
/// and the character is appended to the freeform buffer.
#[test]
fn apply_query_key_append_freeform_clears_selected() {
    let mut qs = make_query_state();
    qs.selected = Some(Count::new(1).inner());
    apply_query_key(&mut qs, &QueryKeyAction::AppendFreeform('h'));
    apply_query_key(&mut qs, &QueryKeyAction::AppendFreeform('i'));
    assert_eq!(qs.freeform.as_str(), "hi");
    assert_eq!(
        qs.selected, None,
        "typing freeform should clear the selection"
    );
}

/// Verifies that SelectDown at the last choice wraps around to the first choice.
///
/// The implementation uses modular arithmetic: (index + 1) % count.
/// Down at index 2 (last of 3 choices) wraps to index 0 (first).
#[test]
fn apply_query_key_select_down_at_end_wraps_to_first() {
    let mut qs = make_query_state();
    qs.selected = Some(Count::new(2).inner()); // last of 3 choices
    apply_query_key(&mut qs, &QueryKeyAction::SelectDown);
    assert_eq!(
        qs.selected,
        Some(Count::new(0).inner()),
        "Down at last choice must wrap to first"
    );
}

/// Verifies that Backspace removes the last character from the freeform buffer.
///
/// Given freeform "hi", one Backspace removes 'i', leaving "h".
/// A second Backspace leaves an empty buffer.
#[test]
fn apply_query_key_backspace_removes_last_char() {
    let mut qs = make_query_state();
    qs.freeform = PromptText::new("hi");
    apply_query_key(&mut qs, &QueryKeyAction::Backspace);
    assert_eq!(
        qs.freeform.as_str(),
        "h",
        "Backspace must pop the last character from freeform"
    );
    apply_query_key(&mut qs, &QueryKeyAction::Backspace);
    assert_eq!(
        qs.freeform.as_str(),
        "",
        "Backspace on single char must leave empty freeform"
    );
}

/// Verifies that the Esc key classifies as KeyAction::CancelThinking.
///
/// Esc is the designated cancel key: pressing it while the agent is thinking
/// should interrupt the in-progress turn, so it must classify as CancelThinking.
#[test]
fn classify_esc_returns_cancel_thinking() {
    let action = classify_key(key(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        matches!(action, KeyAction::CancelThinking),
        "Esc must map to CancelThinking"
    );
}

/// Verifies that apply_key CancelThinking returns false and does not modify state.
///
/// The CancelThinking action is a signal for the TUI actor's dispatch layer
/// to handle. apply_key must be a pure no-op for this variant: it must not
/// modify the prompt buffer, is_thinking flag, or any other state field.
#[test]
fn apply_cancel_thinking_is_noop_in_apply_key() {
    let mut state = default_state();
    state.agent.thinking.is_active = true.into();
    state.prompt.buffer = "something".into();
    state.prompt.cursor = 9;
    let quit = apply_key(&mut state, KeyAction::CancelThinking);
    assert!(
        matches!(quit, ControlFlow::Continue(())),
        "CancelThinking must not return quit=true"
    );
    assert_eq!(
        state.prompt.buffer,
        "something".into(),
        "buffer must be unchanged"
    );
    assert_eq!(state.prompt.cursor, 9, "cursor must be unchanged");
    assert!(
        state.agent.thinking.is_active,
        "is_thinking must be unchanged"
    );
}

// ──────────────────────────────────────────────
// Tab / CompletionUp / CompletionDown tests
// ──────────────────────────────────────────────

use augur_core::actors::command::types::CommandDef;

fn make_cmd(name: &'static str, usage: &'static str) -> CommandDef {
    CommandDef::builder()
        .name(name)
        .usage(usage)
        .description("desc")
        .build()
}

fn model_option(id: &str, display_name: &str) -> augur_tui::domain::types::ModelOption {
    augur_tui::domain::types::ModelOption::builder()
        .id(augur_tui::domain::string_newtypes::ModelId::new(id))
        .display_name(ModelLabel::new(display_name))
        .build()
}

/// Verifies that Tab classifies as KeyAction::ToggleAskFocus.
///
/// Tab toggles input focus between the main chat and the ask panel when the
/// panel is open. Tab completion uses arrow keys + Enter instead.
#[test]
fn classify_tab_is_tab() {
    let action = classify_key(key(KeyCode::Tab, KeyModifiers::NONE));
    assert!(matches!(action, KeyAction::ToggleAskFocus));
}

/// Verifies that Up arrow classifies as KeyAction::CompletionUp.
///
/// In chat mode, Up is only used for completion navigation - no other chat-mode
/// scroll behavior is assigned to it, so it maps directly to CompletionUp.
#[test]
fn classify_up_is_completion_up() {
    let action = classify_key(key(KeyCode::Up, KeyModifiers::NONE));
    assert!(matches!(action, KeyAction::CompletionUp));
}

/// Verifies that Down arrow classifies as KeyAction::CompletionDown.
#[test]
fn classify_down_is_completion_down() {
    let action = classify_key(key(KeyCode::Down, KeyModifiers::NONE));
    assert!(matches!(action, KeyAction::CompletionDown));
}

/// Verifies that Tab is a no-op when no completions are present.
///
/// Pressing Tab with an empty completion list must leave the buffer and cursor
/// unchanged so users cannot accidentally corrupt their typed text.
#[test]
fn tab_on_empty_completions_is_noop() {
    let mut state = default_state();
    state.prompt.buffer = "/q".into();
    state.prompt.cursor = 2;
    let _ = apply_key(&mut state, KeyAction::Tab);
    assert_eq!(state.prompt.buffer, "/q".into());
    assert_eq!(state.prompt.cursor, 2);
}

/// Verifies that Tab applies the selected completion text into the buffer.
///
/// When a completion is highlighted (Some(i)), Tab must copy the command's
/// usage text (with argument placeholders stripped) into the buffer, move
/// the cursor to the end, and clear the completion list.
#[test]
fn tab_applies_selected_completion() {
    let mut state = default_state();
    state.prompt.completions.commands = vec![make_cmd("help", "/help"), make_cmd("quit", "/quit")];
    state.prompt.completions.command_selected = Some(1); // "quit" is selected
    let _ = apply_key(&mut state, KeyAction::Tab);
    assert_eq!(state.prompt.buffer, "/quit".into());
    assert_eq!(state.prompt.cursor, "/quit".len());
    assert!(completions_are_empty(&state.prompt.completions));
    assert_eq!(state.prompt.completions.command_selected, None);
}

/// Verifies that Tab applies the first completion when no selection is active.
///
/// With no item highlighted (None), Tab should complete to the first available
/// option (index 0) rather than doing nothing.
#[test]
fn tab_applies_first_completion_when_none_selected() {
    let mut state = default_state();
    state.prompt.completions.commands = vec![make_cmd("help", "/help"), make_cmd("quit", "/quit")];
    state.prompt.completions.command_selected = None;
    let _ = apply_key(&mut state, KeyAction::Tab);
    assert_eq!(state.prompt.buffer, "/help".into());
    assert!(completions_are_empty(&state.prompt.completions));
}

/// Verifies that Tab strips argument placeholders from the usage string.
///
/// A command with usage "/switch <name>" must complete to "/switch " (with a
/// trailing space) so the user can immediately type the argument without manually
/// deleting the '<name>' placeholder text.
#[test]
fn tab_strips_argument_placeholder() {
    let mut state = default_state();
    state.prompt.completions.commands = vec![make_cmd("switch", "/switch <name>")];
    state.prompt.completions.command_selected = Some(0);
    let _ = apply_key(&mut state, KeyAction::Tab);
    assert_eq!(state.prompt.buffer, "/switch ".into());
    assert_eq!(state.prompt.cursor, "/switch ".len());
}

/// Verifies that CompletionDown from None selects the first item (Some(0)).
///
/// The first Down keypress when nothing is highlighted should move focus to the
/// top of the list, matching the behaviour of common autocomplete UIs.
#[test]
fn completion_down_from_none_selects_first() {
    let mut state = default_state();
    state.prompt.completions.commands = vec![make_cmd("help", "/help"), make_cmd("quit", "/quit")];
    let _ = apply_key(&mut state, KeyAction::CompletionDown);
    assert_eq!(state.prompt.completions.command_selected, Some(0));
}

/// Verifies that CompletionDown at the last item wraps to None.
///
/// Pressing Down past the last item returns to the "no selection" state so the
/// user can exit the list and fall back to the raw buffer text.
#[test]
fn completion_down_at_last_wraps_to_none() {
    let mut state = default_state();
    state.prompt.completions.commands = vec![make_cmd("help", "/help"), make_cmd("quit", "/quit")];
    state.prompt.completions.command_selected = Some(1); // last item
    let _ = apply_key(&mut state, KeyAction::CompletionDown);
    assert_eq!(state.prompt.completions.command_selected, None);
}

/// Verifies that CompletionUp from None selects the last item.
///
/// The first Up keypress when nothing is highlighted should jump to the bottom
/// of the list, matching the reverse-wrap convention of common autocomplete UIs.
#[test]
fn completion_up_from_none_selects_last() {
    let mut state = default_state();
    state.prompt.completions.commands = vec![make_cmd("help", "/help"), make_cmd("quit", "/quit")];
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert_eq!(state.prompt.completions.command_selected, Some(1));
}

/// Verifies that CompletionUp at index 0 wraps to None.
///
/// Pressing Up from the first item returns to the "no selection" state,
/// mirroring the Down-past-last wrapping behavior for symmetry.
#[test]
fn completion_up_at_zero_wraps_to_none() {
    let mut state = default_state();
    state.prompt.completions.commands = vec![make_cmd("help", "/help"), make_cmd("quit", "/quit")];
    state.prompt.completions.command_selected = Some(0);
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert_eq!(state.prompt.completions.command_selected, None);
}

/// Verifies that CompletionDown and CompletionUp are no-ops when completions are empty.
///
/// Navigation actions must not panic or corrupt state when no completions are
/// visible (e.g. user is not in a '/' context).
#[test]
fn completion_navigation_noop_when_empty() {
    let mut state = default_state();
    let _ = apply_key(&mut state, KeyAction::CompletionDown);
    assert_eq!(state.prompt.completions.command_selected, None);
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert_eq!(state.prompt.completions.command_selected, None);
}

/// Verifies that ToolCallStarted pushes a tool-call line with is_tool_call = true.
///
/// The tool-call line must carry the formatted "→ name: arg" summary and be
/// marked as a tool call so the renderer applies dimmed styling.
#[test]
fn apply_tool_call_started_pushes_tool_call_line() {
    let mut state = default_state();
    apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: ToolName::new("list_directory"),
            args: serde_json::json!({ "path": "/tmp" }),
        },
    );
    assert_eq!(state.output.lines.len(), 1);
    assert_eq!(
        state.output.lines[0].kind,
        LineKind::ToolCall,
        "tool call line must be LineKind::ToolCall"
    );
    assert!(
        state.output.lines[0]
            .text
            .as_str()
            .contains("list_directory"),
        "tool call line must mention the tool name"
    );
    assert!(
        state.output.lines[0].text.as_str().contains("/tmp"),
        "tool call line must mention the first argument value"
    );
}

/// Verifies that ToolCallStarted updates thinking_label to "Calling <name>...".
///
/// The thinking row label must reflect the current tool being executed so the
/// user can see which tool is running while the agent is busy.
#[test]
fn apply_tool_call_started_updates_thinking_label() {
    let mut state = default_state();
    state.agent.thinking.is_active = true.into();
    apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: ToolName::new("shell_exec"),
            args: serde_json::json!({ "command": "ls" }),
        },
    );
    assert_eq!(state.agent.thinking.label, "Calling shell_exec...");
}

/// Verifies that UsageUpdate with a model field updates model_display in status.
///
/// When the SDK includes a model name in AssistantUsageData, the TUI must update
/// the status bar model_display so the actual model name is visible after the
/// first turn completes, replacing the config-driven fallback label.
#[test]
fn apply_usage_update_with_model_updates_model_display() {
    use augur_tui::domain::string_newtypes::ModelId;
    let mut state = default_state();
    state.status.model_display = "copilot".into();
    apply_agent_output(
        &mut state,
        AgentOutput::UsageUpdate {
            model: Some(ModelId::new("claude-sonnet-4-5")),
        },
    );
    assert_eq!(state.status.model_display, "claude-sonnet-4-5");
}

/// Verifies that UsageUpdate with model: None leaves model_display unchanged.
///
/// Non-Copilot providers and SDK events that omit the model field must not
/// clear or replace the existing model_display value.
#[test]
fn apply_usage_update_without_model_preserves_model_display() {
    let mut state = default_state();
    state.status.model_display = "gpt-4o".into();
    apply_agent_output(&mut state, AgentOutput::UsageUpdate { model: None });
    assert_eq!(state.status.model_display, "gpt-4o");
}

/// Verifies that ModelsAvailable stores the model list in prompt state.
///
/// The model list is populated at session startup and used by the model picker
/// to display available models when the user types '/model '.
#[test]
fn apply_models_available_stores_models() {
    let mut state = default_state();
    let models = vec![model_option("gemini-3.1-pro", "Gemini 3.1 Pro")];
    apply_agent_output(&mut state, AgentOutput::ModelsAvailable(models.clone()));
    assert_eq!(state.prompt.models.available.len(), 1);
    assert_eq!(
        state.prompt.models.available[0].id.as_str(),
        "gemini-3.1-pro"
    );
    assert_eq!(
        state.prompt.models.available[0].display_name,
        "Gemini 3.1 Pro"
    );
}

/// Verifies that ActiveModelChanged updates the active_id in models state.
///
/// When the Copilot actor reports the active model name, models.active_id must
/// be updated so the model picker can pre-highlight the current model on open.
#[test]
fn apply_active_model_changed_updates_active_id() {
    let mut state = default_state();
    apply_agent_output(&mut state, AgentOutput::ActiveModelChanged("gpt-4o".into()));
    let active_id = state
        .prompt
        .models
        .active_id
        .as_ref()
        .expect("active_id must be set");
    assert_eq!(active_id.as_str(), "gpt-4o");
}

/// Verifies that ActiveModelChanged with empty name sets active_id to Some("").
///
/// An empty model name from the Copilot actor represents auto-selection mode.
/// models.active_id must track this so the picker can pre-select Auto.
#[test]
fn apply_active_model_changed_empty_name_sets_active_id_empty() {
    let mut state = default_state();
    apply_agent_output(&mut state, AgentOutput::ActiveModelChanged("".into()));
    let active_id = state
        .prompt
        .models
        .active_id
        .as_ref()
        .expect("active_id must be set");
    assert_eq!(active_id.as_str(), "");
}

/// Verifies that UsageUpdate with a model field updates models.active_id.
///
/// UsageUpdate carries the model used for a turn. Receiving it must update
/// models.active_id so the model picker reflects the correct active model
/// even when ActiveModelChanged has not yet arrived.
#[test]
fn apply_usage_update_with_model_updates_active_id() {
    use augur_tui::domain::string_newtypes::ModelId;
    let mut state = default_state();
    apply_agent_output(
        &mut state,
        AgentOutput::UsageUpdate {
            model: Some(ModelId::new("claude-3-5-sonnet")),
        },
    );
    let active_id = state
        .prompt
        .models
        .active_id
        .as_ref()
        .expect("active_id must be set");
    assert_eq!(active_id.as_str(), "claude-3-5-sonnet");
}

/// Verifies that ActiveModelChanged updates the model_display string in status.
///
/// After the user selects a model or the Copilot actor reports the session's
/// active model, model_display in the status bar must reflect the new name.
#[test]
fn apply_active_model_changed_updates_model_display() {
    let mut state = default_state();
    state.status.model_display = "copilot".into();
    apply_agent_output(&mut state, AgentOutput::ActiveModelChanged("gpt-4o".into()));
    assert_eq!(state.status.model_display, "gpt-4o");
}

///
/// When the LLM starts producing text after a tool call, the thinking label
/// must revert from "Calling <name>..." back to "Thinking..." so the thinking
/// row reflects the current activity correctly.
#[test]
fn apply_token_resets_thinking_label() {
    let mut state = default_state();
    state.agent.thinking.is_active = true.into();
    state.agent.thinking.label = "Calling some_tool...".into();
    apply_agent_output(&mut state, AgentOutput::Token(OutputText::new("hi")));
    assert_eq!(state.agent.thinking.label, "Thinking...");
}

fn mouse_event(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn output_rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    }
}

/// Verifies that a scroll-up event with the cursor inside the output area produces
/// ScrollUp with the MOUSE_SCROLL_LINES count.
#[test]
fn classify_mouse_scroll_up_in_output_area() {
    let area = output_rect();
    let event = mouse_event(MouseEventKind::ScrollUp, 40, 10);
    let action = classify_mouse(event, area);
    assert!(matches!(action, MouseAction::ScrollUp(n) if n == MOUSE_SCROLL_LINES));
}

/// Verifies that a scroll-down event with the cursor inside the output area produces
/// ScrollDown with the MOUSE_SCROLL_LINES count.
#[test]
fn classify_mouse_scroll_down_in_output_area() {
    let area = output_rect();
    let event = mouse_event(MouseEventKind::ScrollDown, 40, 10);
    let action = classify_mouse(event, area);
    assert!(matches!(action, MouseAction::ScrollDown(n) if n == MOUSE_SCROLL_LINES));
}

/// Verifies that a scroll event with the cursor below the output area bounds is ignored.
///
/// The output area is 20 rows tall; row 25 is outside, so no scroll action occurs.
#[test]
fn classify_mouse_scroll_outside_area_is_ignored() {
    let area = output_rect();
    let event = mouse_event(MouseEventKind::ScrollUp, 40, 25);
    let action = classify_mouse(event, area);
    assert!(matches!(action, MouseAction::Ignored));
}

/// Verifies that a non-scroll mouse event (e.g., cursor movement) is always ignored.
#[test]
fn classify_mouse_non_scroll_event_is_ignored() {
    let area = output_rect();
    let event = mouse_event(MouseEventKind::Moved, 40, 10);
    let action = classify_mouse(event, area);
    assert!(matches!(action, MouseAction::Ignored));
}

/// Verifies that a scroll event at the exact right-edge column of the output area
/// is still treated as inside the area and produces a scroll action.
#[test]
fn classify_mouse_scroll_at_right_edge_of_area() {
    let area = output_rect(); // width 80, so last column is 79
    let event = mouse_event(MouseEventKind::ScrollUp, 79, 10);
    let action = classify_mouse(event, area);
    assert!(matches!(action, MouseAction::ScrollUp(_)));
}

// ── Paste tests ──────────────────────────────────────────────────────────────

/// Verifies that Ctrl+V produces KeyAction::RequestPaste.
#[test]
fn classify_ctrl_v_is_request_paste() {
    let action = classify_key(key(KeyCode::Char('v'), KeyModifiers::CONTROL));
    assert!(matches!(action, KeyAction::RequestPaste));
}

/// Verifies that applying Paste to an empty buffer inserts the full text and
/// advances the cursor to the end of the pasted content.
#[test]
fn apply_paste_inserts_text_into_empty_buffer() {
    let mut state = default_state();
    let _ = apply_key(&mut state, KeyAction::Paste("hello".to_owned()));
    assert_eq!(state.prompt.buffer, "hello".into());
    assert_eq!(state.prompt.cursor, 5);
}

/// Verifies that Paste inserts at the current cursor position, not always at
/// the end, leaving text after the cursor intact.
#[test]
fn apply_paste_inserts_at_cursor_position() {
    let mut state = default_state();
    state.prompt.buffer = "helloworld".into();
    state.prompt.cursor = 5;
    let _ = apply_key(&mut state, KeyAction::Paste(" ".to_owned()));
    assert_eq!(state.prompt.buffer, "hello world".into());
    assert_eq!(state.prompt.cursor, 6);
}

/// Verifies that newline characters in pasted text are replaced with spaces so
/// the single-line prompt buffer does not contain embedded newlines.
#[test]
fn apply_paste_replaces_newlines_with_spaces() {
    let mut state = default_state();
    let _ = apply_key(&mut state, KeyAction::Paste("line1\nline2".to_owned()));
    assert_eq!(state.prompt.buffer, "line1 line2".into());
}

/// Verifies that CRLF sequences in pasted text are replaced with a single space.
#[test]
fn apply_paste_replaces_crlf_with_single_space() {
    let mut state = default_state();
    let _ = apply_key(&mut state, KeyAction::Paste("line1\r\nline2".to_owned()));
    assert_eq!(state.prompt.buffer, "line1 line2".into());
}

/// Verifies that a right mouse button down event produces MouseAction::RightClick
/// regardless of whether the cursor is inside the output area.
#[test]
fn classify_mouse_right_button_down_is_right_click() {
    let area = output_rect();
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 40,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    let action = classify_mouse(event, area);
    assert!(matches!(action, MouseAction::RightClick));
}

/// Verifies that a right mouse button down outside the output area still produces
/// RightClick - paste intent is not restricted to the output zone.
#[test]
fn classify_mouse_right_click_outside_area_is_right_click() {
    let area = output_rect();
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 200,
        row: 200,
        modifiers: KeyModifiers::NONE,
    };
    let action = classify_mouse(event, area);
    assert!(matches!(action, MouseAction::RightClick));
}

// ---------------------------------------------------------------------------
// History navigation tests
// ---------------------------------------------------------------------------

/// Helper: push a user-input line to the output pane as if the user submitted it.
fn push_user_line(state: &mut AppState, text: &str) {
    use augur_tui::domain::newtypes::TimestampMs;
    state.push_user_input_line(OutputText::new(format!("> {}", text)), TimestampMs::new(0));
}

/// Verifies that Up when the buffer is empty and output has user lines loads the most recent entry.
#[test]
fn history_up_empty_buffer_loads_most_recent() {
    let mut state = default_state();
    push_user_line(&mut state, "first");
    push_user_line(&mut state, "second");
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert_eq!(state.prompt.buffer, "second".into());
    assert_eq!(state.prompt.cursor, "second".len());
    assert_eq!(state.prompt.history.pos, Some(0));
}

/// Verifies that pressing Up twice navigates to the second-most-recent entry.
#[test]
fn history_up_twice_reaches_older_entry() {
    let mut state = default_state();
    push_user_line(&mut state, "first");
    push_user_line(&mut state, "second");
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert_eq!(state.prompt.buffer, "first".into());
    assert_eq!(state.prompt.history.pos, Some(1));
}

/// Verifies that Up clamps at the oldest entry and does not go out of bounds.
#[test]
fn history_up_clamps_at_oldest() {
    let mut state = default_state();
    push_user_line(&mut state, "only");
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    let _ = apply_key(&mut state, KeyAction::CompletionUp); // already at oldest
    assert_eq!(state.prompt.buffer, "only".into());
    assert_eq!(state.prompt.history.pos, Some(0));
}

/// Verifies that Down after navigating to the most recent entry restores the empty buffer.
#[test]
fn history_down_from_most_recent_clears_buffer() {
    let mut state = default_state();
    push_user_line(&mut state, "hello");
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    let _ = apply_key(&mut state, KeyAction::CompletionDown);
    assert_eq!(state.prompt.buffer, "".into());
    assert_eq!(state.prompt.history.pos, None);
}

/// Verifies that Down from the middle of history navigates toward the newer entry.
#[test]
fn history_down_from_middle_loads_newer_entry() {
    let mut state = default_state();
    push_user_line(&mut state, "a");
    push_user_line(&mut state, "b");
    push_user_line(&mut state, "c");
    let _ = apply_key(&mut state, KeyAction::CompletionUp); // c
    let _ = apply_key(&mut state, KeyAction::CompletionUp); // b
    let _ = apply_key(&mut state, KeyAction::CompletionUp); // a
    let _ = apply_key(&mut state, KeyAction::CompletionDown); // b
    assert_eq!(state.prompt.buffer, "b".into());
    assert_eq!(state.prompt.history.pos, Some(1));
}

/// Verifies that Up with no user-input lines in the output pane is a no-op.
#[test]
fn history_up_no_entries_is_noop() {
    let mut state = default_state();
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert_eq!(state.prompt.buffer, "".into());
    assert_eq!(state.prompt.history.pos, None);
}

/// Verifies that typing a character resets the history navigation position.
#[test]
fn typing_resets_history_pos() {
    let mut state = default_state();
    push_user_line(&mut state, "prior");
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert!(state.prompt.history.pos.is_some());
    let _ = apply_key(&mut state, KeyAction::AppendChar('x'));
    assert_eq!(state.prompt.history.pos, None);
}

/// Verifies that pasting resets the history navigation position.
#[test]
fn paste_resets_history_pos() {
    let mut state = default_state();
    push_user_line(&mut state, "prior");
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert!(state.prompt.history.pos.is_some());
    let _ = apply_key(&mut state, KeyAction::Paste("pasted".to_owned()));
    assert_eq!(state.prompt.history.pos, None);
}

/// Verifies that Up from a non-empty buffer saves the in-progress text as a draft
/// and navigates to the most recent history entry.
#[test]
fn history_up_nonempty_buffer_saves_draft_and_navigates_to_recent() {
    let mut state = default_state();
    push_user_line(&mut state, "prior command");
    state.prompt.buffer = "in progress".into();
    state.prompt.cursor = "in progress".len();

    let _ = apply_key(&mut state, KeyAction::CompletionUp);

    assert_eq!(state.prompt.buffer, "prior command".into());
    assert_eq!(state.prompt.history.pos, Some(0));
    assert_eq!(state.prompt.history.draft, Some("in progress".to_owned()));
}

/// Verifies that Down from the most recent history entry restores the saved draft.
#[test]
fn history_down_from_most_recent_restores_saved_draft() {
    let mut state = default_state();
    push_user_line(&mut state, "prior command");
    state.prompt.buffer = "in progress".into();
    state.prompt.cursor = "in progress".len();

    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    let _ = apply_key(&mut state, KeyAction::CompletionDown);

    assert_eq!(state.prompt.buffer, "in progress".into());
    assert_eq!(state.prompt.history.pos, None);
    assert_eq!(state.prompt.history.draft, None);
}

/// Verifies that the "> " display prefix is stripped when loading history into the buffer.
#[test]
fn history_strips_display_prefix() {
    let mut state = default_state();
    push_user_line(&mut state, "my command");
    let _ = apply_key(&mut state, KeyAction::CompletionUp);
    assert_eq!(state.prompt.buffer, "my command".into());
}

// ---------------------------------------------------------------------------
// Text-selection mouse action tests
// ---------------------------------------------------------------------------

/// Verifies that a left button Down event inside the output area produces
/// SelectionStart with the event's row and column.
#[test]
fn classify_mouse_left_down_in_area_starts_selection() {
    let area = output_rect();
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 10,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    let action = classify_mouse(event, area);
    assert!(
        matches!(action, MouseAction::SelectionStart { row: 5, col: 10 }),
        "expected SelectionStart{{row:5, col:10}}, got {action:?}"
    );
}

/// Verifies that a left button Down event outside the output area clears the selection.
#[test]
fn classify_mouse_left_down_outside_area_clears_selection() {
    let area = output_rect();
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 200,
        row: 200,
        modifiers: KeyModifiers::NONE,
    };
    let action = classify_mouse(event, area);
    assert!(
        matches!(action, MouseAction::ClearSelection),
        "expected ClearSelection, got {action:?}"
    );
}

/// Verifies that a left-button Drag event inside the output area produces
/// SelectionExtend with the updated cursor position.
#[test]
fn classify_mouse_left_drag_in_area_extends_selection() {
    let area = output_rect();
    let event = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 15,
        row: 8,
        modifiers: KeyModifiers::NONE,
    };
    let action = classify_mouse(event, area);
    assert!(
        matches!(action, MouseAction::SelectionExtend { row: 8, col: 15 }),
        "expected SelectionExtend{{row:8, col:15}}, got {action:?}"
    );
}

/// Verifies that scroll events still produce their expected actions after the
/// selection classification was added (regression guard).
#[test]
fn classify_mouse_scroll_still_works_after_selection_changes() {
    let area = output_rect();
    let up = mouse_event(MouseEventKind::ScrollUp, 40, 10);
    let down = mouse_event(MouseEventKind::ScrollDown, 40, 10);
    assert!(matches!(
        classify_mouse(up, area),
        MouseAction::ScrollUp(..)
    ));
    assert!(matches!(
        classify_mouse(down, area),
        MouseAction::ScrollDown(..)
    ));
}

// ── AgentOutput::Error display tests ─────────────────────────────────────

/// Verifies that apply_agent_output Error places the error text on its own
/// line with is_error = true, not concatenated onto prior output content.
///
/// This is the primary regression guard for the bug where errors were appended
/// to the last existing line, making them invisible when that line had content.
#[test]
fn apply_agent_output_error_is_on_own_line_after_prior_content() {
    let mut state = default_state();
    // Simulate partial LLM response already in the output
    apply_agent_output(&mut state, AgentOutput::Token(OutputText::new("partial")));
    apply_agent_output(
        &mut state,
        AgentOutput::Error(OutputText::new("session failed")),
    );
    // The partial response line must not contain the error text
    assert_eq!(
        state.output.lines[0].text.as_str(),
        "partial",
        "prior content must be untouched"
    );
    // Error must be on its own line
    let error_line = state
        .output
        .lines
        .iter()
        .find(|l| l.kind == LineKind::Error)
        .expect("at least one line must have is_error = true");
    assert_eq!(error_line.text.as_str(), "[error] session failed");
}

/// Verifies that apply_agent_output Error with no prior output creates a
/// new error line without panic or incorrect line count.
///
/// Startup errors (auth failure, JSON-RPC errors) arrive before the user
/// submits any message; the output pane is empty at that point.
#[test]
fn apply_agent_output_error_on_empty_output() {
    let mut state = default_state();
    apply_agent_output(
        &mut state,
        AgentOutput::Error(OutputText::new("auth failed")),
    );
    let error_line = state
        .output
        .lines
        .iter()
        .find(|l| l.kind == LineKind::Error)
        .expect("error line must exist in output");
    assert_eq!(error_line.text.as_str(), "[error] auth failed");
}

/// Verifies that apply_agent_output Error clears is_thinking and pushes two
/// blank separator lines after the error, matching the Done/TurnComplete contract.
///
/// is_thinking must be false after an error so the spinner is not rendered and
/// the user is not left in a "waiting" visual state.
#[test]
fn apply_agent_output_error_clears_thinking_and_pushes_blanks() {
    let mut state = default_state();
    state.agent.thinking.is_active = true.into();
    apply_agent_output(&mut state, AgentOutput::Error(OutputText::new("oops")));
    assert!(
        !state.agent.thinking.is_active,
        "is_thinking must be false after Error"
    );
    // Last two lines must be blank separators from push_turn_end
    let n = state.output.lines.len();
    assert!(n >= 2, "at least error line + 2 blanks expected");
    assert_eq!(state.output.lines[n - 1].text.as_str(), "");
    assert_eq!(state.output.lines[n - 2].text.as_str(), "");
}

/// Verifies that after apply_agent_output Error, subsequent Token output
/// does not get appended to the error line.
///
/// Guards against future regressions where error lines accidentally allow
/// continuation text to be merged in by append_to_last_line.
#[test]
fn apply_agent_output_tokens_after_error_start_fresh_line() {
    let mut state = default_state();
    apply_agent_output(&mut state, AgentOutput::Error(OutputText::new("net error")));
    apply_agent_output(&mut state, AgentOutput::Token(OutputText::new("retry")));
    let error_line = state
        .output
        .lines
        .iter()
        .find(|l| l.kind == LineKind::Error)
        .expect("error line must exist");
    assert_eq!(
        error_line.text.as_str(),
        "[error] net error",
        "error line must not be modified"
    );
    // Token should appear on its own line somewhere after the error
    let token_line = state
        .output
        .lines
        .iter()
        .find(|l| l.text.as_str() == "retry");
    assert!(
        token_line.is_some(),
        "retry token must appear on a separate line"
    );
}

// ── apply_ask_output tests ────────────────────────────────────────────────────

/// Verifies that apply_ask_output appends a token to ask_panel.output when panel is open.
///
/// When ask_panel is Some, a Token event must append its text to the panel's output lines.
#[test]
fn apply_ask_output_appends_token_when_panel_open() {
    use augur_tui::domain::string_newtypes::OutputText;
    use augur_tui::domain::tui_state::AskPanelState;
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    state.interaction.panel.ask_panel = Some(AskPanelState::default());
    apply_ask_output(&mut state, AgentOutput::Token(OutputText::new("hello")));
    let panel = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("panel must remain open");
    let text: String = panel.output.iter().map(|l| l.text.as_str()).collect();
    assert!(
        text.contains("hello"),
        "token must appear in ask panel output; got: {text:?}"
    );
}

/// Verifies that apply_ask_output is a no-op when ask_panel is None.
///
/// When the panel is closed, all AgentOutput variants must be silently discarded.
#[test]
fn apply_ask_output_noop_when_panel_closed() {
    use augur_tui::domain::string_newtypes::OutputText;
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    assert!(state.interaction.panel.ask_panel.is_none());
    apply_ask_output(&mut state, AgentOutput::Token(OutputText::new("ignored")));
    assert!(
        state.interaction.panel.ask_panel.is_none(),
        "panel must stay None"
    );
    assert!(
        state.output.lines.is_empty()
            || state
                .output
                .lines
                .iter()
                .all(|l| !l.text.as_str().contains("ignored")),
        "token must not appear in main output"
    );
}

/// Verifies that apply_ask_output clears ask_panel.thinking on TurnComplete.
///
/// When TurnComplete arrives while panel is open, thinking flag must be set to false.
#[test]
fn apply_ask_output_clears_thinking_on_turn_complete() {
    use augur_tui::domain::tui_state::AskPanelState;
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    let panel = AskPanelState {
        thinking: IsThinking::yes(),
        ..AskPanelState::default()
    };
    state.interaction.panel.ask_panel = Some(panel);
    apply_ask_output(&mut state, AgentOutput::TurnComplete);
    let p = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("panel stays open");
    assert!(!p.thinking, "thinking must be false after TurnComplete");
}

/// Verifies that apply_ask_output clears ask_panel.thinking on Done.
///
/// Done is the non-SDK equivalent of TurnComplete; both must clear the thinking flag.
#[test]
fn apply_ask_output_clears_thinking_on_done() {
    use augur_tui::domain::tui_state::AskPanelState;
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    let panel = AskPanelState {
        thinking: IsThinking::yes(),
        ..AskPanelState::default()
    };
    state.interaction.panel.ask_panel = Some(panel);
    apply_ask_output(&mut state, AgentOutput::Done);
    let p = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("panel stays open");
    assert!(!p.thinking, "thinking must be false after Done");
}

/// Verifies that apply_ask_output clears ask_panel.thinking on Error.
///
/// Error output ends the ask turn; thinking must be cleared so the spinner stops.
#[test]
fn apply_ask_output_clears_thinking_on_error() {
    use augur_tui::domain::string_newtypes::{OutputText, StringNewtype};
    use augur_tui::domain::tui_state::AskPanelState;
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    let panel = AskPanelState {
        thinking: IsThinking::yes(),
        ..AskPanelState::default()
    };
    state.interaction.panel.ask_panel = Some(panel);
    apply_ask_output(&mut state, AgentOutput::Error(OutputText::new("boom")));
    let p = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("panel stays open");
    assert!(!p.thinking, "thinking must be false after Error");
}

/// Verifies that apply_ask_output pushes error text to ask_panel.output on Error.
///
/// When the ask turn errors (e.g. unknown endpoint), the error message must appear
/// in the panel output so the user sees the failure rather than a silent spinner stop.
#[test]
fn apply_ask_output_shows_error_text_in_panel() {
    use augur_tui::domain::string_newtypes::{OutputText, StringNewtype};
    use augur_tui::domain::tui_state::{AskPanelState, LineKind};
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    let panel = AskPanelState {
        thinking: IsThinking::yes(),
        ..AskPanelState::default()
    };
    state.interaction.panel.ask_panel = Some(panel);
    apply_ask_output(
        &mut state,
        AgentOutput::Error(OutputText::new("unknown endpoint")),
    );
    let p = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("panel stays open");
    assert!(!p.thinking, "thinking must be false after Error");
    let has_error_text = p
        .output
        .iter()
        .any(|l| l.text.as_str().contains("unknown endpoint"));
    assert!(
        has_error_text,
        "error text must appear in panel output; got: {:?}",
        p.output.iter().map(|l| l.text.as_str()).collect::<Vec<_>>()
    );
    let has_error_kind = p.output.iter().any(|l| matches!(l.kind, LineKind::Error));
    assert!(
        has_error_kind,
        "at least one line must have LineKind::Error"
    );
}

/// Verifies that apply_ask_output pushes a blank line when MessageBreak arrives.
///
/// MessageBreak separates multi-part assistant replies; the ask panel must insert
/// a blank output line to give visual breathing room, matching main-output behaviour.
#[test]
fn apply_ask_output_message_break_pushes_blank_line() {
    use augur_tui::domain::tui_state::AskPanelState;
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    let mut panel = AskPanelState::default();
    panel
        .output
        .push(augur_tui::domain::tui_state::OutputLine::plain(
            augur_tui::domain::string_newtypes::OutputText::new("existing"),
        ));
    state.interaction.panel.ask_panel = Some(panel);
    apply_ask_output(&mut state, AgentOutput::MessageBreak);
    let p = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("panel stays open");
    assert!(
        p.output.len() >= 2,
        "MessageBreak must push at least one blank line; got {} lines",
        p.output.len()
    );
    let last = p.output.last().expect("must have lines");
    assert!(
        last.text.as_str().is_empty(),
        "last line after MessageBreak must be blank; got: {:?}",
        last.text.as_str()
    );
}

/// Verifies that apply_ask_output pushes blank separator lines after Done.
///
/// Done ends the AI turn; the ask panel must push blank lines as separators so
/// the next user message appears visually distinct, matching the main output behaviour.
#[test]
fn apply_ask_output_done_pushes_separator_lines() {
    use augur_tui::domain::tui_state::AskPanelState;
    use augur_tui::domain::types::AgentOutput;
    let mut state = default_state();
    let mut panel = AskPanelState {
        thinking: IsThinking::yes(),
        ..AskPanelState::default()
    };
    panel
        .output
        .push(augur_tui::domain::tui_state::OutputLine::plain(
            augur_tui::domain::string_newtypes::OutputText::new("response"),
        ));
    let initial_len = 1usize;
    state.interaction.panel.ask_panel = Some(panel);
    apply_ask_output(&mut state, AgentOutput::Done);
    let p = state
        .interaction
        .panel
        .ask_panel
        .as_ref()
        .expect("panel stays open");
    assert!(!p.thinking, "thinking must be false after Done");
    assert!(
        p.output.len() > initial_len,
        "Done must push blank separator lines; got {} lines (initial was {})",
        p.output.len(),
        initial_len
    );
}

// ── apply_agent_feed_output tests ────────────────────────────────────────────

/// Verifies that apply_agent_feed_output TaskStarted sets active_task.
///
/// When a TaskStarted event is received, 'agent_feed.active_task' must be set
/// to the provided task name so the thinking row can display it.
#[test]
fn apply_agent_feed_output_task_started_sets_active_task() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    assert!(state.interaction.panel.agent_feed.active_task.is_none());
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskStarted {
            name: "deploy".into(),
            model: None,
        },
    );
    assert_eq!(
        state.interaction.panel.agent_feed.active_task.as_deref(),
        Some("deploy"),
        "TaskStarted must set active_task to the provided name",
    );
}

/// Verifies that apply_agent_feed_output TaskStarted captures the current active model.
///
/// When a TaskStarted event is received with an active model, 'agent_feed.current_agent_model'
/// must be set to that model so the label can display it (e.g., "[ claude-haiku-4.5 ]").
#[test]
fn apply_agent_feed_output_task_started_captures_model_name() {
    use augur_tui::domain::string_newtypes::ModelId;
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();
    state.prompt.models.active_id = Some(ModelId::new("claude-haiku-4.5"));
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .is_none()
    );

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskStarted {
            name: "deploy".into(),
            model: None,
        },
    );

    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .is_some(),
        "TaskStarted must capture the current active model"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .as_deref(),
        Some("claude-haiku-4.5"),
        "captured model must match active_id"
    );
}

/// Verifies that apply_agent_feed_output TaskStarted without active model leaves current_agent_model None.
///
/// When TaskStarted is received but no model is active, 'current_agent_model' should remain None.
#[test]
fn apply_agent_feed_output_task_started_no_model_leaves_none() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();
    assert!(state.prompt.models.active_id.is_none());
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .is_none()
    );

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskStarted {
            name: "deploy".into(),
            model: None,
        },
    );

    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .is_none(),
        "current_agent_model must remain None when no active model"
    );
}

/// Verifies that apply_agent_feed_output TaskStarted with a step model uses that model.
///
/// When TaskStarted carries 'model: Some("claude-sonnet-4.6")', 'current_agent_model'
/// must be set to that value rather than the conversation model from 'state.prompt.models.active_id'.
#[test]
fn apply_agent_feed_output_task_started_step_model_overrides_conversation_model() {
    use augur_tui::domain::string_newtypes::ModelId;
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();
    // Set a different conversation model to confirm it is NOT used.
    state.prompt.models.active_id = Some(ModelId::new("gpt-4o"));

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskStarted {
            name: "plan-builder".into(),
            model: Some(ModelLabel::new("claude-sonnet-4.6")),
        },
    );

    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .as_deref(),
        Some("claude-sonnet-4.6"),
        "step model must override conversation model when provided in TaskStarted"
    );
}

/// Verifies that apply_agent_feed_output StatusLine appends to agent_feed.output.
///
/// Each StatusLine event must append exactly one output line to the feed.
#[test]
fn apply_agent_feed_output_status_line_appends_to_output() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    assert!(state.interaction.panel.agent_feed.output.is_empty());
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine(OutputText::new("step 1 done".to_owned())),
    );
    // StatusLine is now buffered, not immediately in output
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .is_some(),
        "StatusLine must be buffered in pending_status_message"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "StatusLine must not immediately append to output (should be buffered)"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .as_ref()
            .map(|l| l.text.as_str())
            .unwrap_or(""),
        "step 1 done",
    );
}

/// Verifies that apply_agent_feed_output Clear empties output and clears active_task.
///
/// Clear must reset the feed to empty state so stale output is not displayed
/// after a new task session starts.
#[test]
fn apply_agent_feed_output_clear_empties_output_and_task() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    state.interaction.panel.agent_feed.active_task = Some("old-task".into());
    state.interaction.panel.agent_feed.output.push(
        augur_tui::domain::tui_state::OutputLine::plain(OutputText::new("old line".to_owned())),
    );
    apply_agent_feed_output(&mut state, AgentFeedOutput::Clear);
    assert!(
        state.interaction.panel.agent_feed.output.is_empty(),
        "Clear must empty the output vec",
    );
    assert!(
        state.interaction.panel.agent_feed.active_task.is_none(),
        "Clear must set active_task to None",
    );
}

/// Verifies that apply_agent_feed_output Clear also clears the current_agent_model.
#[test]
fn apply_agent_feed_output_clear_clears_model() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    state.interaction.panel.agent_feed.current_agent_model = Some("claude-haiku-4.5".into());

    apply_agent_feed_output(&mut state, AgentFeedOutput::Clear);

    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .is_none(),
        "Clear must set current_agent_model to None",
    );
}

/// Verifies that apply_agent_feed_output TaskCompleted appends a completion line and clears active_task.
#[test]
fn apply_agent_feed_output_task_completed_appends_line_and_clears_task() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    state.interaction.panel.agent_feed.active_task = Some("deploy".into());
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "deploy".into(),
        },
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        1,
        "must append exactly one line on TaskCompleted"
    );
    assert!(
        state.interaction.panel.agent_feed.output[0]
            .text
            .as_str()
            .contains("deploy"),
        "completion line must contain the task name"
    );
    assert!(
        state.interaction.panel.agent_feed.active_task.is_none(),
        "active_task must be cleared on TaskCompleted"
    );
}

/// Verifies that apply_agent_feed_output TaskCompleted clears the current_agent_model.
#[test]
fn apply_agent_feed_output_task_completed_clears_model() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();
    state.interaction.panel.agent_feed.active_task = Some("deploy".into());
    state.interaction.panel.agent_feed.current_agent_model = Some("claude-haiku-4.5".into());

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "deploy".into(),
        },
    );

    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .is_none(),
        "current_agent_model must be cleared on TaskCompleted"
    );
}

/// Verifies that apply_agent_feed_output TaskFailed appends an error line and clears active_task.
#[test]
fn apply_agent_feed_output_task_failed_pushes_error_line_and_clears_task() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    state.interaction.panel.agent_feed.active_task = Some("build".into());
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskFailed {
            name: "build".into(),
            reason: "compilation error".into(),
        },
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        1,
        "must append exactly one error line on TaskFailed"
    );
    let line = &state.interaction.panel.agent_feed.output[0];
    assert_eq!(
        line.kind,
        LineKind::Error,
        "TaskFailed must produce an Error-kind line"
    );
    assert!(
        line.text.as_str().contains("build"),
        "error line must contain the task name"
    );
    assert!(
        line.text.as_str().contains("compilation error"),
        "error line must contain the failure reason"
    );
    assert!(
        state.interaction.panel.agent_feed.active_task.is_none(),
        "active_task must be cleared on TaskFailed"
    );
}

/// Verifies that apply_agent_feed_output TaskFailed clears the current_agent_model.
#[test]
fn apply_agent_feed_output_task_failed_clears_model() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();
    state.interaction.panel.agent_feed.active_task = Some("build".into());
    state.interaction.panel.agent_feed.current_agent_model = Some("claude-opus-4.7".into());

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskFailed {
            name: "build".into(),
            reason: "compilation error".into(),
        },
    );

    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .current_agent_model
            .is_none(),
        "current_agent_model must be cleared on TaskFailed"
    );
}

/// Verifies that apply_agent_feed_output ToolEventLine produces separate output lines, not accumulated.
///
/// When consecutive ToolEventLine events arrive (tool start, progress, complete),
/// each must be buffered in pending_tool_event (replacing the previous one).
/// Tool events are only pushed to output when flushed by a structural event
/// (StatusLine, TaskStarted, TaskCompleted, TaskFailed, or Clear).
#[test]
fn apply_agent_feed_output_tool_events_do_not_accumulate() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    // Apply first tool event (start)
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine(OutputText::new("→ tool_name: doing something".to_owned())),
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "ToolEventLine must be buffered, not immediately output"
    );
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_tool_event
            .is_some(),
        "first ToolEventLine must buffer in pending_tool_event"
    );

    // Apply second tool event (progress) - replaces first
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine(OutputText::new("Progressing...".to_owned())),
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "second ToolEventLine must replace buffered event (not add to output)"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_tool_event
            .as_ref()
            .unwrap()
            .text
            .as_str(),
        "Progressing...",
        "pending_tool_event must contain the latest tool event"
    );

    // Apply third tool event (complete) - replaces second
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine(OutputText::new("✓ tool_name".to_owned())),
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "third ToolEventLine must replace buffered event (not add to output)"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_tool_event
            .as_ref()
            .unwrap()
            .text
            .as_str(),
        "✓ tool_name",
        "pending_tool_event must contain the latest tool event"
    );

    // StatusLine must NOT flush buffered tool event - tool calls don't interrupt streamed messages
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("streaming message chunk".into()),
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "StatusLine must not flush pending tool event to output (no interruption of streaming messages)"
    );
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_tool_event
            .is_some(),
        "pending_tool_event must remain buffered after StatusLine"
    );
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .is_some(),
        "pending_status_message must be set after StatusLine"
    );

    // TaskCompleted DOES flush both buffers: tool event first, then status message
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "test-task".into(),
        },
    );
    // output: [tool_event, status_message, task_completed] = 3 lines
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        3,
        "TaskCompleted must flush tool event + status message + push completed line"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output[0].text.as_str(),
        "✓ tool_name",
        "tool event must be committed before status message"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output[1].text.as_str(),
        "streaming message chunk",
        "status message must be committed after tool event"
    );
    assert!(
        state.interaction.panel.agent_feed.output[2]
            .text
            .as_str()
            .contains("test-task"),
        "final line must be the task-completed message"
    );
}

/// Verifies that consecutive StatusLine events accumulate into a single pending message.
///
/// Streaming delta chunks (emitted as StatusLine every ~200 chars) must append to the
/// same pending_status_message entry rather than creating separate output lines.
/// This ensures the agent panel shows one cohesive growing message, not many short lines.
#[test]
fn apply_agent_feed_output_consecutive_status_lines_accumulate_into_one_pending() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("chunk one ".into()));
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("chunk two ".into()));
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("chunk three".into()),
    );

    // All three chunks must be in ONE pending entry, not in output
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "consecutive StatusLine events must not produce committed output lines"
    );
    let pending = state
        .interaction
        .panel
        .agent_feed
        .buffers
        .pending_status_message
        .as_ref()
        .expect("pending_status_message must exist after StatusLine events");
    assert_eq!(
        pending.text.as_str(),
        "chunk one chunk two chunk three",
        "all chunks must be concatenated in the single pending entry"
    );
}

/// Verifies that ToolEventLine during streaming is not flushed by StatusLine events.
///
/// Tool calls arriving between streaming delta chunks must stay buffered in
/// pending_tool_event and not interrupt the growing message. They are committed
/// only at structural boundaries (TaskCompleted, TaskFailed, TaskStarted, Clear).
#[test]
fn apply_agent_feed_output_tool_event_stays_buffered_through_status_lines() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    // A tool event arrives during streaming
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("→ bash: compile".into()),
    );
    // A new streaming chunk arrives after the tool event
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("next message chunk".into()),
    );

    // Tool event must still be buffered - StatusLine must not flush it
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "StatusLine must not flush pending_tool_event to output"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_tool_event
            .as_ref()
            .map(|l| l.text.as_str())
            .unwrap_or(""),
        "→ bash: compile",
        "pending_tool_event must remain unchanged after StatusLine"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .as_ref()
            .map(|l| l.text.as_str())
            .unwrap_or(""),
        "next message chunk",
        "StatusLine content must be in pending_status_message"
    );
}

// ── MessageBreak tests ────────────────────────────────────────────────────────

/// Verifies that MessageBreak flushes 'pending_status_message' to committed output.
///
/// After streaming chunks have accumulated in 'pending_status_message', a
/// 'MessageBreak' must commit that entry to 'output' so the completed message
/// appears as a permanent line in the feed.
#[test]
fn apply_agent_feed_output_message_break_flushes_pending_status() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("hello ".into()));
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("world".into()));

    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "status chunks must be pending before MessageBreak"
    );

    apply_agent_feed_output(&mut state, AgentFeedOutput::MessageBreak);

    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        1,
        "MessageBreak must flush pending_status_message to output"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output[0].text.as_str(),
        "hello world",
        "flushed line must contain all accumulated chunks"
    );
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .is_none(),
        "pending_status_message must be cleared after MessageBreak"
    );
}

/// Verifies that MessageBreak flushes a buffered 'pending_tool_event' to output.
///
/// Tool events arrive between streaming delta chunks and are held in
/// 'pending_tool_event' to avoid interleaving with in-flight message text.
/// 'MessageBreak' (end of the message) must commit the buffered tool event.
#[test]
fn apply_agent_feed_output_message_break_flushes_pending_tool_event() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("→ bash: compile".into()),
    );

    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "tool event must be pending before MessageBreak"
    );

    apply_agent_feed_output(&mut state, AgentFeedOutput::MessageBreak);

    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        1,
        "MessageBreak must flush pending_tool_event to output"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output[0].text.as_str(),
        "→ bash: compile"
    );
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_tool_event
            .is_none(),
        "pending_tool_event must be cleared after MessageBreak"
    );
}

/// Verifies that MessageBreak commits status before tool event when both are pending.
///
/// The correct flush order is: 'pending_status_message' first, then
/// 'pending_tool_event'. This preserves the original event ordering: streamed
/// message text appears before the tool call that followed it.
#[test]
fn apply_agent_feed_output_message_break_flushes_status_before_tool_event() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("agent reply".into()),
    );
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("→ bash: run".into()),
    );

    apply_agent_feed_output(&mut state, AgentFeedOutput::MessageBreak);

    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        2,
        "MessageBreak must flush both buffers: status message and tool event"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output[0].text.as_str(),
        "agent reply",
        "status message must be committed first"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output[1].text.as_str(),
        "→ bash: run",
        "tool event must be committed second"
    );
}

/// Verifies that MessageBreak is a no-op when both pending buffers are empty.
#[test]
fn apply_agent_feed_output_message_break_noop_when_no_pending() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    apply_agent_feed_output(&mut state, AgentFeedOutput::MessageBreak);

    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "MessageBreak on empty buffers must not produce any output"
    );
}

/// Verifies that a second StatusLine after MessageBreak starts a fresh pending entry.
///
/// After 'MessageBreak' flushes the first message, subsequent 'StatusLine' chunks
/// must begin accumulating into a new 'pending_status_message' entry, not append
/// to the already-committed line.
#[test]
fn apply_agent_feed_output_status_after_message_break_starts_new_pending() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();

    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("first message".into()),
    );
    apply_agent_feed_output(&mut state, AgentFeedOutput::MessageBreak);
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("second message".into()),
    );

    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        1,
        "only the first message should be in committed output"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output[0].text.as_str(),
        "first message"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .as_ref()
            .map(|l| l.text.as_str())
            .unwrap_or(""),
        "second message",
        "second StatusLine must be pending, not appended to committed first message"
    );
}

// ── auto-open panel and CloseSecondaryPanel tests ─────────────────────────────
/// Verifies that apply_agent_feed_output auto-opens AgentFeed panel when no secondary panel is open.
///
/// When 'secondary_view' is 'None' and any 'AgentFeedOutput' arrives,
/// 'secondary_view' must be set to 'Some(AgentFeed)' so the panel appears automatically.
#[test]
fn apply_agent_feed_output_auto_opens_panel_when_secondary_closed() {
    use augur_tui::domain::tui_state::SecondaryView;
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    assert!(state.interaction.panel.secondary_view.is_none());
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine(OutputText::new("hello".to_owned())),
    );
    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::AgentFeed),
        "apply_agent_feed_output must auto-open AgentFeed panel when secondary_view is None",
    );
}

/// Verifies that apply_agent_feed_output does not steal focus from an open Ask panel.
///
/// When 'secondary_view' is 'Some(Ask)' and an 'AgentFeedOutput' arrives,
/// 'secondary_view' must remain 'Some(Ask)' - the feed panel must not steal focus.
#[test]
fn apply_agent_feed_output_does_not_steal_ask_when_ask_open() {
    use augur_tui::domain::tui_state::SecondaryView;
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine(OutputText::new("background update".to_owned())),
    );
    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::Ask),
        "apply_agent_feed_output must not replace an already-open Ask panel",
    );
}

/// Verifies that CloseSecondaryPanel key action maps correctly from Ctrl+W.
#[test]
fn ctrl_w_maps_to_close_secondary_panel() {
    let action = classify_key(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert!(
        matches!(action, KeyAction::CloseSecondaryPanel),
        "Ctrl+W must map to CloseSecondaryPanel; got {action:?}",
    );
}

/// Verifies that Ctrl+O maps to agent feed navigation left.
#[test]
fn ctrl_o_maps_to_agent_feed_prev() {
    let action = classify_key(key(KeyCode::Char('o'), KeyModifiers::CONTROL));
    assert!(
        matches!(action, KeyAction::AgentFeedPrev),
        "Ctrl+O must map to AgentFeedPrev; got {action:?}",
    );
}

/// Verifies that Ctrl+P maps to agent feed navigation right.
#[test]
fn ctrl_p_maps_to_agent_feed_next() {
    let action = classify_key(key(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(
        matches!(action, KeyAction::AgentFeedNext),
        "Ctrl+P must map to AgentFeedNext; got {action:?}",
    );
}

/// Verifies that apply_agent_feed_output with TaskStarted populates agent_feed active_task.
///
/// Calling apply_agent_feed_output with TaskStarted("step-1") must set
/// agent_feed.active_task to Some("step-1").
#[test]
fn supervisor_step_started_populates_agent_feed() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskStarted {
            name: "step-1".into(),
            model: None,
        },
    );
    assert_eq!(
        state.interaction.panel.agent_feed.active_task.as_deref(),
        Some("step-1"),
        "TaskStarted must set active_task to step-1",
    );
}

/// Verifies that apply_agent_feed_output with StatusLine buffers the message.
///
/// Calling apply_agent_feed_output with StatusLine("All steps complete.")
/// must buffer the message instead of immediately appending to output.
#[test]
fn supervisor_execution_complete_appends_status_line() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine(OutputText::new("All steps complete.".to_owned())),
    );
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .is_some(),
        "StatusLine must be buffered in pending_status_message"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "StatusLine must not immediately append to output"
    );
    assert_eq!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .as_ref()
            .map(|l| l.text.as_str())
            .unwrap_or(""),
        "All steps complete.",
        "Buffered line must contain the exact status message",
    );
}

// ── timestamp regression tests ────────────────────────────────────────────────

/// Verifies that apply_agent_feed_output StatusLine sets header.timestamp.
///
/// Every StatusLine buffered in the agent feed must carry a timestamp so the
/// renderer can display the '[HH:MM:SS]' prefix on each message.
#[test]
fn apply_agent_feed_output_status_line_has_timestamp() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine(OutputText::new("running tool".to_owned())),
    );
    let line = state
        .interaction
        .panel
        .agent_feed
        .buffers
        .pending_status_message
        .as_ref()
        .expect("StatusLine must be buffered in pending_status_message");
    assert!(
        line.header.timestamp.is_some(),
        "StatusLine must have header.timestamp set, got None"
    );
}

/// Verifies that apply_agent_feed_output TaskCompleted sets header.timestamp.
///
/// The completion line pushed when a task finishes must carry a timestamp.
#[test]
fn apply_agent_feed_output_task_completed_has_timestamp() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "my-agent".into(),
        },
    );
    let line = &state.interaction.panel.agent_feed.output[0];
    assert!(
        line.header.timestamp.is_some(),
        "TaskCompleted line must have header.timestamp set, got None"
    );
}

/// Verifies that apply_agent_feed_output TaskFailed sets header.timestamp.
///
/// The error line pushed when a task fails must carry a timestamp.
#[test]
fn apply_agent_feed_output_task_failed_has_timestamp() {
    use augur_tui::domain::types::AgentFeedOutput;
    let mut state = default_state();
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskFailed {
            name: "my-agent".into(),
            reason: "out of memory".into(),
        },
    );
    let line = &state.interaction.panel.agent_feed.output[0];
    assert!(
        line.header.timestamp.is_some(),
        "TaskFailed line must have header.timestamp set, got None"
    );
}

/// Verifies that 'ToolCallStarted' preserves tool name and args in OutputLine metadata.
#[test]
fn apply_agent_output_tool_call_started_preserves_metadata() {
    use augur_tui::domain::string_newtypes::ToolName;
    use augur_tui::domain::tui_state::LineKind;
    use augur_tui::domain::types::AgentOutput;

    let mut state = default_state();
    let initial_line_count = state.output.lines.len();

    let tool_name = ToolName::new("view");
    let tool_args = serde_json::json!({ "path": "/src/main.rs" });

    apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name.clone(),
            args: tool_args.clone(),
        },
    );

    // Verify a new line was added
    let new_line_count = state.output.lines.len();
    assert!(
        new_line_count > initial_line_count,
        "ToolCallStarted must add at least one line"
    );

    // Find the ToolCall line
    let tool_line = state
        .output
        .lines
        .iter()
        .find(|line| line.kind == LineKind::ToolCall)
        .expect("must have a ToolCall line after ToolCallStarted event");

    // Verify metadata is populated and correct
    let metadata = tool_line
        .metadata
        .as_ref()
        .expect("ToolCall line must have metadata from ToolCallStarted");
    assert_eq!(
        metadata.tool_name.as_str(),
        "view",
        "tool_name in metadata must match event"
    );
    assert_eq!(
        metadata.tool_args.get("path").and_then(|v| v.as_str()),
        Some("/src/main.rs"),
        "tool_args in metadata must be preserved"
    );
}

/// Verifies that tool metadata is accessible at render time without panics.
#[test]
fn apply_agent_output_tool_metadata_accessible_at_render_time() {
    use augur_tui::domain::string_newtypes::ToolName;
    use augur_tui::domain::tui_state::LineKind;
    use augur_tui::domain::types::AgentOutput;

    let mut state = default_state();

    let tool_name = ToolName::new("grep");
    let tool_args = serde_json::json!({
        "pattern": "TODO",
        "path": "/src"
    });

    apply_agent_output(
        &mut state,
        AgentOutput::ToolCallStarted {
            name: tool_name,
            args: tool_args,
        },
    );

    // Find the ToolCall line and verify metadata is accessible
    let tool_line = state
        .output
        .lines
        .iter()
        .find(|line| line.kind == LineKind::ToolCall)
        .expect("must have a ToolCall line");

    // Verify we can access metadata fields without unwrap panicking
    if let Some(metadata) = &tool_line.metadata {
        let _tool_name_str: &str = metadata.tool_name.as_str();
        let _tool_args_obj: &serde_json::Value = &metadata.tool_args;
        // If we reach here without panic, metadata is accessible
    } else {
        panic!("ToolCall line must have metadata");
    }
}

/// Verifies that 'TaskCompleted' flushes the buffer to output.
#[test]
fn apply_agent_feed_output_status_line_buffer_flush_on_task_completed() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Create a pending status message
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("Processing".into()));

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_status_message.is_some(),
        "StatusLine must create buffer"
    );
    assert!(feed.output.is_empty(), "Output must be empty before flush");

    // Complete task, which should flush buffer
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "my-task".into(),
        },
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_status_message.is_none(),
        "Buffer must be cleared after flush"
    );
    assert!(
        !feed.output.is_empty(),
        "Output must contain at least the TaskCompleted line"
    );

    // Verify the original status line is in output
    let status_line_found = feed
        .output
        .iter()
        .any(|line| line.text.as_str().contains("Processing"));
    assert!(
        status_line_found,
        "Flushed StatusLine text must be in output"
    );
}

/// Verifies that consecutive StatusLine events accumulate into one pending message.
///
/// When multiple 'StatusLine' events arrive, each chunk is appended to the single
/// pending buffer rather than flushing the previous chunk to output. On 'TaskCompleted'
/// the accumulated buffer is flushed as one line, followed by the completion row.
#[test]
fn apply_agent_feed_output_token_chunks_each_get_own_output_row() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("I".into()));
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("'ve".into()));
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine(" successfully".into()),
    );

    // After 3 StatusLines: all accumulated into one pending, nothing in output.
    {
        let feed = &state.interaction.panel.agent_feed;
        assert_eq!(
            feed.output.len(),
            0,
            "StatusLine events must not produce committed output lines"
        );
        let pending = feed
            .buffers
            .pending_status_message
            .as_ref()
            .expect("pending_status_message must be Some after StatusLine events");
        assert_eq!(
            pending.text.as_str(),
            "I've successfully",
            "all chunks must be concatenated in the single pending entry"
        );
    }

    // Flush everything by completing the task.
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "test".into(),
        },
    );

    // output[0]: accumulated StatusLine text; output[1]: TaskCompleted.
    let feed = &state.interaction.panel.agent_feed;
    assert_eq!(
        feed.output.len(),
        2,
        "TaskCompleted must flush accumulated status message and push completed line"
    );
    assert_eq!(feed.output[0].text.as_str(), "I've successfully");
}

/// Verifies that 'Clear' event flushes and clears the buffer.
#[test]
fn apply_agent_feed_output_status_line_buffer_cleared_on_clear_event() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Create a pending status message
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("Message".into()));

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_status_message.is_some(),
        "StatusLine must create buffer"
    );

    // Send Clear event
    apply_agent_feed_output(&mut state, AgentFeedOutput::Clear);

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_status_message.is_none(),
        "Buffer must be cleared after Clear event"
    );
    assert!(
        feed.output.is_empty(),
        "Output must be empty after Clear event"
    );
    assert!(
        feed.active_task.is_none(),
        "Active task must be cleared after Clear event"
    );
}

/// Verifies that 'ToolEventLine' events are buffered instead of immediately output.
///
/// When a 'ToolEventLine' event arrives, it must be stored in 'pending_tool_event'
/// instead of being immediately pushed to 'output'. This prevents tool event lines
/// from interleaving with 'StatusLine' messages that are still being streamed.
#[test]
fn apply_agent_feed_output_tool_event_is_buffered() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("Running deploy step...".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "ToolEventLine must be buffered in pending_tool_event"
    );
    assert!(
        feed.output.is_empty(),
        "ToolEventLine must not be immediately pushed to output"
    );
}

/// Verifies that 'StatusLine' does NOT flush the pending tool event buffer.
///
/// When a 'StatusLine' event arrives after a 'ToolEventLine', the buffered tool
/// event must remain in 'pending_tool_event'. Tool events are committed only at
/// structural boundaries (TaskCompleted, TaskFailed, TaskStarted, Clear).
#[test]
fn apply_agent_feed_output_status_line_flushes_tool_buffer() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("Tool event".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "ToolEventLine must be buffered"
    );
    assert!(feed.output.is_empty(), "Output must be empty");

    // Send a StatusLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("Status message".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "Tool buffer must remain buffered when StatusLine arrives (not flushed)"
    );
    assert_eq!(
        feed.output.len(),
        0,
        "Output must be empty - StatusLine must not flush the tool event"
    );
    assert!(
        feed.buffers.pending_status_message.is_some(),
        "StatusLine must now be buffered"
    );
}

/// Verifies that 'TaskCompleted' flushes the pending tool event buffer.
///
/// When a 'TaskCompleted' event arrives, any buffered tool event must be
/// flushed to output before the completion message is added.
#[test]
fn apply_agent_feed_output_task_completed_flushes_tool_buffer() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("Tool running".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "ToolEventLine must be buffered"
    );

    // Complete task
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "deploy".into(),
        },
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_none(),
        "Tool buffer must be flushed on TaskCompleted"
    );
    assert_eq!(
        feed.output.len(),
        2,
        "Output must contain tool event and completion message"
    );
}

/// Verifies that 'TaskFailed' flushes the pending tool event buffer.
///
/// When a 'TaskFailed' event arrives, any buffered tool event must be
/// flushed to output before the error message is added.
#[test]
fn apply_agent_feed_output_task_failed_flushes_tool_buffer() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("Tool error detected".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "ToolEventLine must be buffered"
    );

    // Fail task
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskFailed {
            name: "deploy".into(),
            reason: "Deployment failed".into(),
        },
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_none(),
        "Tool buffer must be flushed on TaskFailed"
    );
    assert_eq!(
        feed.output.len(),
        2,
        "Output must contain tool event and error message"
    );
}

/// Verifies that 'TaskStarted' flushes the pending tool event buffer.
///
/// When a 'TaskStarted' event arrives, any buffered tool event must be
/// flushed to output first to maintain proper ordering of events.
#[test]
fn apply_agent_feed_output_task_started_flushes_tool_buffer() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("First tool event".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "ToolEventLine must be buffered"
    );
    assert!(feed.active_task.is_none(), "No active task yet");

    // Start a new task
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskStarted {
            name: "step-2".into(),
            model: None,
        },
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_none(),
        "Tool buffer must be flushed on TaskStarted"
    );
    assert_eq!(
        feed.output.len(),
        1,
        "Output must contain the flushed tool event"
    );
    assert_eq!(
        feed.active_task.as_deref(),
        Some("step-2"),
        "Active task must be set"
    );
}

/// Verifies that 'Clear' flushes the pending tool event buffer.
///
/// When a 'Clear' event arrives, any buffered tool event must be
/// flushed to output before the feed is cleared.
#[test]
fn apply_agent_feed_output_clear_flushes_tool_buffer() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("Tool event".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "ToolEventLine must be buffered"
    );

    // Clear the feed
    apply_agent_feed_output(&mut state, AgentFeedOutput::Clear);

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_none(),
        "Tool buffer must be cleared"
    );
    assert!(
        feed.output.is_empty(),
        "Output must be empty after Clear (flushed then cleared)"
    );
}

/// Verifies that multiple consecutive 'ToolEventLine' events don't break ordering.
///
/// When multiple tool events arrive in sequence, each must replace the previous
/// buffer entry (since only one tool event is buffered at a time). When a flush
/// event arrives, only the most recent tool event is output.
#[test]
fn apply_agent_feed_output_consecutive_tool_events_use_latest() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send first ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("First tool event".into()),
    );

    // Send second ToolEventLine (replaces first)
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("Second tool event".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    assert!(
        feed.buffers.pending_tool_event.is_some(),
        "Tool buffer should contain the second event"
    );
    assert!(feed.output.is_empty(), "Output should still be empty");

    // StatusLine does NOT flush - tool event stays buffered
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("Status".into()));

    let feed = &state.interaction.panel.agent_feed;
    assert_eq!(
        feed.output.len(),
        0,
        "StatusLine must not flush pending_tool_event to output"
    );
    assert_eq!(
        feed.buffers
            .pending_tool_event
            .as_ref()
            .map(|l| l.text.as_str())
            .unwrap_or(""),
        "Second tool event",
        "pending_tool_event must still hold the latest tool event after StatusLine"
    );
}

/// Verifies that tool event buffer has a timestamp.
///
/// Every buffered tool event must have a timestamp set so that when it is
/// flushed to output, it carries timing information.
#[test]
fn apply_agent_feed_output_tool_event_has_timestamp() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a ToolEventLine
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::ToolEventLine("Tool event".into()),
    );

    let feed = &state.interaction.panel.agent_feed;
    let buffered = feed.buffers.pending_tool_event.as_ref();
    assert!(buffered.is_some(), "Tool event must be buffered");
    assert!(
        buffered.unwrap().header.timestamp.is_some(),
        "Buffered tool event must have a timestamp"
    );
}

/// Verifies that consecutive StatusLine events accumulate into one pending entry.
///
/// Each 'StatusLine' appends to the single pending buffer instead of flushing the
/// previous chunk. The result is one accumulated pending entry for the full streamed
/// message, committed only at a structural boundary.
#[test]
fn apply_agent_feed_output_status_line_buffering_regression() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("Loading".into()));
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("...".into()));
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("complete".into()));

    // All three chunks must be accumulated into one pending entry, nothing in output.
    let feed = &state.interaction.panel.agent_feed;
    assert_eq!(
        feed.output.len(),
        0,
        "StatusLine events must not produce committed output lines"
    );
    let buffered = feed
        .buffers
        .pending_status_message
        .as_ref()
        .expect("pending buffer must be Some after StatusLine events")
        .text
        .as_str();
    assert_eq!(
        buffered, "Loading...complete",
        "all chunks must be concatenated in the single pending entry"
    );
}

/// Verifies that when a buffered StatusLine contains '\n', flushing produces separate
/// output lines for each segment.
///
/// Newline characters within a StatusLine are split into multiple output lines on flush.
/// The first segment inherits the original header (timestamp); subsequent segments are
/// plain lines with no timestamp.
#[test]
fn agent_feed_newline_in_status_line_splits_on_flush() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    // Send a StatusLine containing newlines - it stays buffered until flushed.
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::StatusLine("Line one\nLine two\nLine three".into()),
    );

    // Not yet flushed - still in pending buffer.
    assert!(
        state
            .interaction
            .panel
            .agent_feed
            .buffers
            .pending_status_message
            .is_some(),
        "StatusLine with newlines must remain in the pending buffer until a structural event"
    );
    assert_eq!(
        state.interaction.panel.agent_feed.output.len(),
        0,
        "No output lines must be created before flush"
    );

    // Flush by sending a TaskCompleted event.
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskCompleted {
            name: "test-task".into(),
        },
    );

    let feed = &state.interaction.panel.agent_feed;
    // Expect: "Line one", "Line two", "Line three" (from the status split) + 1 completion line.
    assert_eq!(
        feed.output.len(),
        4,
        "Three newline-delimited segments plus one TaskCompleted line must be in output"
    );
    assert_eq!(
        feed.output[0].text.as_str(),
        "Line one",
        "First segment must be the first output line"
    );
    assert_eq!(
        feed.output[1].text.as_str(),
        "Line two",
        "Second segment must be the second output line"
    );
    assert_eq!(
        feed.output[2].text.as_str(),
        "Line three",
        "Third segment must be the third output line"
    );
}

/// Verifies that consecutive StatusLine events accumulate into one pending entry.
///
/// Each 'StatusLine' appends to the single pending buffer. A subsequent structural
/// event (TaskStarted) flushes the accumulated buffer as one output row.
/// The result is one distinct output row for the full streamed message.
#[test]
fn agent_feed_consecutive_status_lines_each_produce_own_output_row() {
    use augur_tui::domain::types::AgentFeedOutput;

    let mut state = default_state();

    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("Step A".into()));
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine(" -> ".into()));
    apply_agent_feed_output(&mut state, AgentFeedOutput::StatusLine("Step B".into()));

    // After 3 StatusLines: all accumulated into one pending, nothing in output.
    {
        let feed = &state.interaction.panel.agent_feed;
        assert_eq!(
            feed.output.len(),
            0,
            "StatusLine events must not produce committed output lines"
        );
        let buffered_text = feed
            .buffers
            .pending_status_message
            .as_ref()
            .expect("pending_status_message must be Some after StatusLine events")
            .text
            .as_str();
        assert_eq!(
            buffered_text, "Step A -> Step B",
            "all chunks must be concatenated in the single pending entry"
        );
    }

    // TaskStarted flushes the pending buffer.
    apply_agent_feed_output(
        &mut state,
        AgentFeedOutput::TaskStarted {
            name: "next-task".into(),
            model: None,
        },
    );

    let feed = &state.interaction.panel.agent_feed;
    // output[0]: "Step A -> Step B" (flushed by TaskStarted).
    // TaskStarted updates active_task metadata but does not push a visible output row.
    assert_eq!(
        feed.output.len(),
        1,
        "TaskStarted must flush the accumulated pending message as one output row"
    );
    assert_eq!(feed.output[0].text.as_str(), "Step A -> Step B");
}
