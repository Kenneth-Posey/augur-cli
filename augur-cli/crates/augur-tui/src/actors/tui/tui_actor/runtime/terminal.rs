//! Terminal-event helpers for the TUI actor runtime.

use crate::actors::tui::assistant::clipboard::{
    extend_selection, paste_from_clipboard, start_selection,
};
use crate::actors::tui::assistant::key_dispatch::{
    dispatch_chat_key, dispatch_guided_plan_key, dispatch_plan_esc, dispatch_query_key,
};
use crate::actors::tui::assistant::plan_view::handle_plan_mouse_scroll;
use crate::domain::tui_input::{MouseAction, classify_mouse, insert_paste};
use crate::domain::tui_state::{AppState, ConversationMode, SelectionPoint};
use augur_domain::domain::string_newtypes::PromptText;
use ratatui::layout::Rect;
use std::ops::ControlFlow;
use tokio::sync::watch;

use super::{super::EventOutcome, super::TuiHandles};
use crate::actors::tui::handle::ShutdownSignal;

/// Handle one terminal event from crossterm and map it to an event-loop outcome.
pub(super) async fn handle_terminal_event(
    state: &mut AppState,
    maybe_event: Option<Result<crossterm::event::Event, std::io::Error>>,
    handles: &TuiHandles<'_>,
) -> EventOutcome {
    let Some(Ok(event)) = maybe_event else {
        return EventOutcome::Quit;
    };
    handle_terminal_ok_event(state, event, handles).await
}

/// Handle a mouse event in conversation or plan mode.
pub(in crate::actors::tui::tui_actor) fn handle_mouse_event(
    state: &mut AppState,
    event: crossterm::event::MouseEvent,
) -> EventOutcome {
    if in_plan_mode(state) {
        handle_plan_mouse_scroll(state, event);
        return EventOutcome::Redraw;
    }
    if let Some(outcome) = handle_secondary_panel_mouse(state, event) {
        return outcome;
    }
    handle_main_panel_mouse(state, event)
}

fn in_plan_mode(state: &AppState) -> bool {
    matches!(state.interaction.mode, ConversationMode::Plan(_))
}

fn handle_main_panel_mouse(
    state: &mut AppState,
    event: crossterm::event::MouseEvent,
) -> EventOutcome {
    let action = classify_mouse(event, state.output.panel_areas.output_area.get());
    if apply_main_panel_mouse_action(state, action) {
        EventOutcome::Redraw
    } else {
        EventOutcome::NoOp
    }
}

/// Handle mouse events within the secondary panel (agent feed or ask view).
///
/// Returns `Some(EventOutcome)` if the event was handled within the secondary panel
/// (e.g., scrolling when bounds are met). Returns `None` if the event occurred outside
/// the secondary panel's bounds, allowing it to fall through to main panel handling.
fn handle_secondary_panel_mouse(
    state: &mut AppState,
    event: crossterm::event::MouseEvent,
) -> Option<EventOutcome> {
    // Extract the secondary panel area
    let area = get_secondary_panel_area(state)?;

    // Classify the mouse action within secondary bounds
    apply_secondary_panel_mouse_action(state, classify_mouse_in_secondary_panel(event, area))
}

/// Get the secondary panel area if it has non-zero dimensions.
fn get_secondary_panel_area(state: &AppState) -> Option<Rect> {
    let area = state.output.panel_areas.secondary_panel_area.get();
    if area.width > 0 && area.height > 0 {
        Some(area)
    } else {
        None
    }
}

/// Classify a mouse event when it occurs within the secondary panel.
/// Returns only scroll and right-click actions when the mouse is within bounds;
/// returns `Ignored` otherwise to delegate to main panel handling.
fn classify_mouse_in_secondary_panel(
    event: crossterm::event::MouseEvent,
    area: Rect,
) -> MouseAction {
    use crossterm::event::{MouseButton, MouseEventKind};

    // Always handle right-clicks
    if matches!(event.kind, MouseEventKind::Down(MouseButton::Right)) {
        return MouseAction::RightClick;
    }

    if !is_mouse_in_bounds(event, area) {
        return MouseAction::Ignored;
    }
    classify_secondary_scroll(event.kind)
}

async fn handle_terminal_ok_event(
    state: &mut AppState,
    event: crossterm::event::Event,
    handles: &TuiHandles<'_>,
) -> EventOutcome {
    match event {
        crossterm::event::Event::Key(key) => key_outcome_from_dispatch(state, key, handles).await,
        crossterm::event::Event::Mouse(mouse) => handle_mouse_event(state, mouse),
        other => handle_non_input_terminal_event(state, other),
    }
}

fn handle_non_input_terminal_event(
    state: &mut AppState,
    event: crossterm::event::Event,
) -> EventOutcome {
    match event {
        crossterm::event::Event::Paste(text) => {
            insert_paste(&mut state.prompt, PromptText::from(text));
            EventOutcome::Redraw
        }
        crossterm::event::Event::Resize(_, _) => EventOutcome::Redraw,
        _ => EventOutcome::NoOp,
    }
}

async fn key_outcome_from_dispatch(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    handles: &TuiHandles<'_>,
) -> EventOutcome {
    if matches!(
        dispatch_key_for_mode(state, key, handles).await,
        ControlFlow::Break(())
    ) {
        EventOutcome::Quit
    } else {
        EventOutcome::Redraw
    }
}

fn apply_main_panel_mouse_action(state: &mut AppState, action: MouseAction) -> bool {
    if apply_main_panel_scroll_or_paste(state, &action) {
        return true;
    }
    if matches!(action, MouseAction::Ignored) {
        return false;
    }
    apply_main_panel_selection_action(state, action);
    true
}

fn apply_main_panel_scroll_or_paste(state: &mut AppState, action: &MouseAction) -> bool {
    match action {
        MouseAction::ScrollUp(n) => {
            state.scroll_up(augur_domain::domain::newtypes::Count::of(*n));
            true
        }
        MouseAction::ScrollDown(n) => {
            state.scroll_down(augur_domain::domain::newtypes::Count::of(*n));
            true
        }
        MouseAction::RightClick => {
            paste_from_clipboard(state);
            true
        }
        _ => false,
    }
}

fn apply_main_panel_selection_action(state: &mut AppState, action: MouseAction) {
    match action {
        MouseAction::SelectionStart { row, col } => {
            start_selection(state, SelectionPoint { row, col });
        }
        MouseAction::SelectionExtend { row, col } => {
            extend_selection(state, SelectionPoint { row, col });
        }
        MouseAction::ClearSelection => state.output.selection = None,
        _ => {}
    }
}

fn apply_secondary_panel_mouse_action(
    state: &mut AppState,
    action: MouseAction,
) -> Option<EventOutcome> {
    match action {
        MouseAction::ScrollUp(n) => {
            state.agent_feed_scroll_up(augur_domain::domain::newtypes::Count::of(n))
        }
        MouseAction::ScrollDown(n) => {
            state.agent_feed_scroll_down(augur_domain::domain::newtypes::Count::of(n))
        }
        MouseAction::RightClick => paste_from_clipboard(state),
        MouseAction::SelectionStart { .. }
        | MouseAction::SelectionExtend { .. }
        | MouseAction::ClearSelection
        | MouseAction::Ignored => return None,
    }
    Some(EventOutcome::Redraw)
}

fn classify_secondary_scroll(kind: crossterm::event::MouseEventKind) -> MouseAction {
    use crossterm::event::MouseEventKind;

    match kind {
        MouseEventKind::ScrollUp => {
            MouseAction::ScrollUp(crate::domain::tui_input::MOUSE_SCROLL_LINES)
        }
        MouseEventKind::ScrollDown => {
            MouseAction::ScrollDown(crate::domain::tui_input::MOUSE_SCROLL_LINES)
        }
        _ => MouseAction::Ignored,
    }
}

/// Check if a mouse event occurred within the given rectangular bounds.
fn is_mouse_in_bounds(event: crossterm::event::MouseEvent, area: Rect) -> bool {
    event.column >= area.x
        && event.column < area.x + area.width
        && event.row >= area.y
        && event.row < area.y + area.height
}

async fn dispatch_key_for_mode(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    match state.interaction.mode {
        ConversationMode::Query(_) => dispatch_query_key(state, key),
        ConversationMode::GuidedPlan(_) => dispatch_guided_plan_key(state, key, handles).await,
        ConversationMode::Plan(_) => dispatch_plan_key(state, key, handles).await,
        _ => dispatch_chat_key(state, key, handles).await,
    }
}

async fn dispatch_plan_key(
    state: &mut AppState,
    key: crossterm::event::KeyEvent,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    if is_plan_exit_key(key) && dispatch_plan_esc(state).is_some() {
        return ControlFlow::Continue(());
    }
    dispatch_chat_key(state, key, handles).await
}

fn is_plan_exit_key(key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyEventKind};

    key.kind == KeyEventKind::Press && key.code == KeyCode::Esc
}

/// Restore the terminal and notify waiters that shutdown has completed.
pub(super) fn shutdown_runtime(shutdown_tx: watch::Sender<ShutdownSignal>) {
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableBracketedPaste,
        crossterm::event::DisableMouseCapture,
    );
    ratatui::restore();
    let _ = shutdown_tx.send(ShutdownSignal::Complete);
}
