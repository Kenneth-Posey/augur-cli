//! Key dispatch helpers: chat key handling, submit, cancel, completions, and query dispatch.

mod completion;
mod panel;
mod submit;

use super::clipboard::{copy_selection_if_c_pressed, paste_from_clipboard};
use super::plan_view::handle_query_submit;
use crate::actors::tui::tui_actor::TuiHandles;
use crate::domain::tui_input::{
    apply_key, apply_query_key, classify_key, classify_query_key, push_turn_end, KeyAction,
    QueryKeyAction,
};
use crate::domain::tui_state::{AppState, ConversationMode, InputFocus, SecondaryView};
use augur_domain::domain::string_newtypes::OutputText;
use std::ops::ControlFlow;

const FORCE_ADVANCE_FKEY: u8 = 10;

pub(crate) use completion::refresh_completion_hints;
pub(crate) use completion::refresh_file_hints;
pub use completion::{apply_selected_completion, close_completions_if_open, refresh_model_hints};
pub(crate) use panel::{dispatch_plan_esc, toggle_ask_focus};
use panel::{handle_ask_submit, toggle_agent_feed_view, toggle_ask_view};
pub(crate) use submit::handle_submit;

/// Handle a key event in normal chat mode. Returns `true` on quit.
///
/// Classifies the key, applies it to state, and delegates submit/cancel
/// to `handle_cancel_or_submit`. `RequestPaste` reads the OS clipboard.
/// After every keypress the relevant completion hint list is refreshed:
/// buffer starts with `/` → command hints; buffer contains `@` → file hints;
/// otherwise both lists are cleared.
/// `ShiftTab` cycles the secondary view (None→Ask→close). `ToggleAgentFeed`
/// cycles the agent feed view (None→AgentFeed→close). `ToggleAskFocus`
/// switches input focus between Main and Ask when the ask panel is open.
///
/// Consumers: `dispatch_key_for_mode` in `actor.rs` for non-query keypresses.
pub(crate) async fn dispatch_chat_key(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    if apply_completion_on_tab(state, key, handles)
        || copy_selection_if_c_pressed(state, key).is_some()
    {
        return ControlFlow::Continue(());
    }
    let action = classify_key(key);
    if let ControlFlow::Break(()) = apply_key(state, action.clone()) {
        return ControlFlow::Break(());
    }
    if handle_immediate_chat_action(state, &action, handles).await {
        return ControlFlow::Continue(());
    }
    if !should_skip_completion_refresh(state, &action) {
        refresh_completion_hints(state, handles);
    }
    maybe_handle_turn_action(state, action, handles).await
}

fn should_skip_completion_refresh(state: &AppState, action: &KeyAction) -> bool {
    matches!(action, KeyAction::CompletionUp | KeyAction::CompletionDown)
        && state.prompt.history.pos.is_some()
}

/// Decide whether to cancel the current turn, submit a new one, or do nothing.
///
/// For Esc with completions open: closes the completion list and returns false.
/// A second Esc then interrupts normally.
/// For Enter with a completion selected: applies the completion text before submit.
/// When `input_focus` is `Ask`, Esc switches focus to Main without interrupting
/// the agent. When `input_focus` is `Main` and `ask_panel` is open, Esc closes
/// the panel without interrupting the agent.
/// When `input_focus` is `Ask`, Enter routes to `handle_ask_submit` instead of
/// the main agent.
///
/// | is_cancel | focus | panel | action |
/// |-----------|-------|-------|--------|
/// | true      | Ask   | Some  | set focus=Main, return false |
/// | true      | Main  | Some  | close panel, return false |
/// | true      | *     | None  | existing cancel logic |
/// | false     | Ask   | *     | handle_ask_submit, return false |
/// | false     | Main  | *     | existing submit logic |
///
/// Returns `true` when `handle_submit` signals a quit command.
///
/// Consumers: `dispatch_chat_key` in this module.
pub(crate) async fn handle_cancel_or_submit(
    state: &mut AppState,
    action: KeyAction,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    let is_cancel = matches!(action, KeyAction::CancelThinking);
    if consume_cancel_overlay(state, is_cancel) || prepare_submit_target(state, handles, is_cancel)
    {
        return ControlFlow::Continue(());
    }
    match next_turn_action(
        is_cancel,
        state.agent.thinking.is_active.into(),
        !state.prompt.buffer.is_empty(),
    ) {
        TurnAction::InterruptOnly => {
            handles.agent.interrupt();
            push_turn_end(state, Some(OutputText::from("[stopped]")));
            ControlFlow::Continue(())
        }
        TurnAction::SubmitAfterInterrupt => {
            handles.agent.interrupt();
            push_turn_end(state, Some(OutputText::from("[steering]")));
            handle_submit(state, handles).await
        }
        TurnAction::Submit => handle_submit(state, handles).await,
        TurnAction::NoOp => ControlFlow::Continue(()),
    }
}

/// Handle a key event in query overlay mode. Returns `true` on quit.
///
/// Classifies the key as a `QueryKeyAction`. Submit calls `handle_query_submit`.
/// Quit returns `true`. All other actions are applied to the `QueryState` in place.
///
/// Consumers: `dispatch_key_for_mode` in `actor.rs` when in `ConversationMode::Query`.
pub(crate) fn dispatch_query_key(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
) -> ControlFlow<()> {
    let action = classify_query_key(key);
    match action {
        QueryKeyAction::Quit => ControlFlow::Break(()),
        QueryKeyAction::Submit => {
            handle_query_submit(state);
            ControlFlow::Continue(())
        }
        other => {
            if let ConversationMode::Query(ref mut qs) = state.interaction.mode {
                apply_query_key(qs, &other);
            }
            ControlFlow::Continue(())
        }
    }
}

/// Handle a key event in guided plan mode. Returns `true` on quit.
///
/// Intercepts F10 to force-advance past a `NeedsRework` gate and Enter with an
/// empty prompt buffer to confirm the current phase. All other keypresses delegate
/// to `dispatch_chat_key` so the user retains full chat interaction during plan
/// execution.
///
/// Consumers: `dispatch_key_for_mode` in `actor.rs` when in `ConversationMode::GuidedPlan`.
pub(crate) async fn dispatch_guided_plan_key(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    use crossterm::event::{KeyCode, KeyEventKind};
    if key.kind != KeyEventKind::Press {
        return ControlFlow::Continue(());
    }
    let buffer_empty = state.prompt.buffer.is_empty();
    match (key.code, buffer_empty) {
        (KeyCode::F(FORCE_ADVANCE_FKEY), _) => {
            handles.tools.guided_plan.force_advance();
            ControlFlow::Continue(())
        }
        (KeyCode::Enter, true) => {
            handles.tools.guided_plan.confirm_phase();
            ControlFlow::Continue(())
        }
        _ => dispatch_chat_key(state, key, handles).await,
    }
}

fn apply_completion_on_tab(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    handles: &TuiHandles<'_>,
) -> bool {
    let is_plain_tab =
        matches!(key.code, crossterm::event::KeyCode::Tab) && key.modifiers.is_empty();
    if !is_plain_tab {
        return false;
    }
    let has_file_completion = !state.prompt.completions.files.is_empty();
    let has_any_completion = has_file_completion
        || !state.prompt.completions.commands.is_empty()
        || !state.prompt.completions.model_picker.items.is_empty();
    if !has_any_completion {
        return false;
    }
    crate::domain::tui_input::apply_tab_completion(state);
    // Refresh only after file completion so the hint list reflects the updated path.
    // Command and model completions clear themselves via apply_tab_completion; a
    // subsequent refresh would re-populate them from the buffer, which is unwanted.
    if has_file_completion {
        refresh_completion_hints(state, handles);
    }
    true
}

async fn handle_immediate_chat_action(
    state: &mut AppState,
    action: &KeyAction,
    handles: &TuiHandles<'_>,
) -> bool {
    match action {
        KeyAction::ShiftTab => {
            toggle_ask_view(state, handles).await;
            true
        }
        KeyAction::ToggleAgentFeed => {
            toggle_agent_feed_view(state);
            true
        }
        KeyAction::AgentFeedPrev => {
            state.select_prev_agent_feed();
            true
        }
        KeyAction::AgentFeedNext => {
            state.select_next_agent_feed();
            true
        }
        KeyAction::CloseSecondaryPanel => {
            close_secondary_panel(state);
            true
        }
        KeyAction::ToggleAskFocus => {
            toggle_ask_focus(state);
            true
        }
        KeyAction::RequestPaste => {
            paste_from_clipboard(state);
            false
        }
        _ => false,
    }
}

async fn maybe_handle_turn_action(
    state: &mut AppState,
    action: KeyAction,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    if matches!(action, KeyAction::Submit | KeyAction::CancelThinking) {
        return handle_cancel_or_submit(state, action, handles).await;
    }
    ControlFlow::Continue(())
}

fn consume_cancel_overlay(state: &mut AppState, is_cancel: bool) -> bool {
    if !is_cancel {
        return false;
    }
    if close_completions_if_open(state).is_some() {
        return true;
    }
    if state.interaction.panel.secondary_view.is_some() {
        close_secondary_panel(state);
        return true;
    }
    false
}

fn prepare_submit_target(state: &mut AppState, handles: &TuiHandles<'_>, is_cancel: bool) -> bool {
    if is_cancel {
        return false;
    }
    if state.interaction.panel.input_focus == InputFocus::Ask {
        let ask_is_visible = matches!(
            state.interaction.panel.secondary_view,
            Some(SecondaryView::Ask)
        ) && state.interaction.panel.ask_panel.is_some();
        if ask_is_visible {
            handle_ask_submit(state, handles);
            return true;
        }
        // Defensive normalization: hidden/stale Ask focus must never steal Enter.
        state.interaction.panel.input_focus = InputFocus::Main;
    }
    apply_selected_completion(state);
    false
}

fn close_secondary_panel(state: &mut AppState) {
    state.interaction.panel.secondary_view = None;
    state.interaction.panel.input_focus = InputFocus::Main;
}

enum TurnAction {
    InterruptOnly,
    SubmitAfterInterrupt,
    Submit,
    NoOp,
}

fn next_turn_action(is_cancel: bool, is_thinking: bool, has_text: bool) -> TurnAction {
    match (is_cancel, is_thinking, has_text) {
        (true, true, _) => TurnAction::InterruptOnly,
        (true, false, _) => TurnAction::NoOp,
        (false, true, true) => TurnAction::SubmitAfterInterrupt,
        (false, true, false) => TurnAction::NoOp,
        (false, false, _) => TurnAction::Submit,
    }
}
