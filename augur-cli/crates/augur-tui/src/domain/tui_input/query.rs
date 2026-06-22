//! Query-overlay input helpers.

use super::*;
use augur_domain::domain::newtypes::TextCharacter;

/// Apply a query-overlay key action to the active query state.
pub fn apply_query_key(state: &mut QueryState, action: &QueryKeyAction) {
    if matches!(action, QueryKeyAction::SelectDown) {
        advance_selection_down(state);
        return;
    }
    if matches!(action, QueryKeyAction::SelectUp) {
        advance_selection_up(state);
        return;
    }
    apply_query_text_edit(state, action);
}

fn apply_query_text_edit(state: &mut QueryState, action: &QueryKeyAction) {
    match action {
        QueryKeyAction::AppendFreeform(c) => {
            state.freeform.push(TextCharacter(*c));
            state.selected = None;
        }
        QueryKeyAction::Backspace => {
            state.freeform.pop();
        }
        _ => {}
    }
}

fn advance_selection_down(state: &mut QueryState) {
    let n = state.choices.len();
    if n == 0 {
        return;
    }
    state.selected = Some(match state.selected {
        None => 0,
        Some(i) => (i + 1) % n,
    });
}

fn advance_selection_up(state: &mut QueryState) {
    let n = state.choices.len();
    if n == 0 {
        return;
    }
    state.selected = Some(match state.selected {
        None => n - 1,
        Some(0) => n - 1,
        Some(i) => i - 1,
    });
}
