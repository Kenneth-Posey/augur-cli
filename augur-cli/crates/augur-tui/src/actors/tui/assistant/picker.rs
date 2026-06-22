//! Session picker event handling: key dispatch, session restore, and mode transitions.

use super::session_restore::apply_restored_session;
use crate::actors::tui::tui_actor::TuiHandles;
use crate::domain::tui_input::{PickerKeyAction, apply_picker_key, classify_picker_key};
use crate::domain::tui_state::{AppScreen, AppState, ConversationMode, PickerState};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::persistence::store;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

static FORCE_SESSION_LOAD_PANIC: AtomicBool = AtomicBool::new(false);

fn set_force_session_load_panic(force: bool) {
    FORCE_SESSION_LOAD_PANIC.store(force, Ordering::SeqCst);
}

fn load_selected_session(
    dir: &std::path::Path,
    id: &augur_domain::domain::string_newtypes::SessionId,
) -> anyhow::Result<augur_domain::persistence::types::SessionRecord> {
    if FORCE_SESSION_LOAD_PANIC.load(Ordering::SeqCst) {
        std::panic::panic_any("forced picker session load panic");
    }
    store::load_session(dir, id)
}

/// Process one terminal event while in `AppScreen::SessionSelector`.
///
/// Extracts the key from the event and dispatches it as a `PickerKeyAction`.
/// Returns `true` when a quit action is received or the event stream ends.
///
/// Consumers: `run` event loop in `actor.rs` when in `SessionSelector` screen.
pub(crate) async fn handle_picker_event(
    state: &mut AppState,
    maybe_event: Option<Result<crossterm::event::Event, std::io::Error>>,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    let key = match maybe_event {
        None | Some(Err(_)) => return ControlFlow::Break(()),
        Some(Ok(crossterm::event::Event::Key(key))) => key,
        Some(Ok(_)) => return ControlFlow::Continue(()),
    };
    let action = classify_picker_key(key);
    dispatch_picker_action(state, action, handles).await
}

/// Apply a `PickerKeyAction` to the current state, returning true on quit.
///
/// Handles all picker variants: quit exits, new session clears the picker,
/// ignored events are no-ops, selection keys mutate the highlighted row,
/// and confirm triggers `restore_session`.
///
/// Consumers: `handle_picker_event` in this module.
pub(crate) async fn dispatch_picker_action(
    state: &mut AppState,
    action: PickerKeyAction,
    handles: &TuiHandles<'_>,
) -> ControlFlow<()> {
    match action {
        PickerKeyAction::Quit => ControlFlow::Break(()),
        PickerKeyAction::NewSession => {
            handles.persistence.reset_to_new_session();
            handles.agent.replace_session(None);
            state.reset_for_new_session();
            state.interaction.screen = AppScreen::Conversation;
            state.interaction.mode = ConversationMode::Chat;
            ControlFlow::Continue(())
        }
        PickerKeyAction::Ignored => ControlFlow::Continue(()),
        PickerKeyAction::SelectUp | PickerKeyAction::SelectDown => {
            if let AppScreen::SessionSelector(ref mut ps) = state.interaction.screen {
                apply_picker_key(ps, &action);
            }
            ControlFlow::Continue(())
        }
        PickerKeyAction::Delete => {
            delete_selected_session(state, handles).await;
            ControlFlow::Continue(())
        }
        PickerKeyAction::Confirm => {
            if let Some(picker) = state.take_picker_state() {
                restore_session(state, picker, handles).await;
            }
            ControlFlow::Continue(())
        }
    }
}

async fn delete_selected_session(state: &mut AppState, handles: &TuiHandles<'_>) {
    let (selected_idx, selected_id) = match &state.interaction.screen {
        AppScreen::SessionSelector(ps) => {
            let idx = ps.selected.inner();
            let Some(summary) = ps.sessions.get(idx) else {
                return;
            };
            (idx, summary.identity.id.clone())
        }
        AppScreen::Conversation => return,
    };

    let dir = handles.persistence.sessions_dir();
    let join_result =
        tokio::task::spawn_blocking(move || store::delete_session(&dir, &selected_id)).await;
    let delete_result = match join_result {
        Ok(r) => r,
        Err(e) => Err(anyhow::anyhow!("task panicked: {e}")),
    };

    match delete_result {
        Ok(()) => {
            if let AppScreen::SessionSelector(ref mut ps) = state.interaction.screen {
                if selected_idx < ps.sessions.len() {
                    ps.sessions.remove(selected_idx);
                }
                let new_idx = selected_idx.min(ps.sessions.len().saturating_sub(1));
                ps.selected = Count::of(new_idx);
            }
        }
        Err(e) => {
            state.push_output_token(OutputText::new(format!(
                "[error] failed to delete session: {e}"
            )));
            state.push_output_newline();
        }
    }
}

/// Load and apply a saved session, transitioning the TUI to chat mode.
///
/// Reads the selected session file from disk via a blocking spawn, updates
/// the persistence handle, restores the LLM endpoint on the session actor,
/// and sends the message history to the agent actor. On any error, pushes
/// an error line to the output and enters chat mode anyway.
///
/// Consumers: `dispatch_picker_action` in this module.
pub(crate) async fn restore_session(
    state: &mut AppState,
    picker: PickerState,
    handles: &TuiHandles<'_>,
) {
    let Some(summary) = picker.sessions.get(picker.selected.inner()) else {
        state.interaction.screen = AppScreen::Conversation;
        state.interaction.mode = ConversationMode::Chat;
        return;
    };
    let id = summary.identity.id.clone();
    let dir = handles.persistence.sessions_dir();
    let join_result = tokio::task::spawn_blocking(move || load_selected_session(&dir, &id)).await;
    let load_result = match join_result {
        Ok(r) => r,
        Err(e) => Err(anyhow::anyhow!("task panicked: {e}")),
    };
    match load_result {
        Err(e) => {
            state.push_output_token(OutputText::new(format!(
                "[error] failed to load session: {e}"
            )));
            state.push_output_newline();
            state.interaction.screen = AppScreen::Conversation;
            state.interaction.mode = ConversationMode::Chat;
        }
        Ok(record) => apply_restored_session(state, record, handles).await,
    }
}
