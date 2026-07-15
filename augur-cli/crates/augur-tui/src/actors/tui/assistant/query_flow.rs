//! Query lifecycle helpers for entering query mode and submitting answers.

use crate::domain::tui_state::{AppState, ConversationMode, QueryState, current_timestamp_ms};
use augur_domain::domain::string_newtypes::{ChoiceText, OutputText, PromptText, StringNewtype};
use augur_domain::tools::builtin::query_user::QueryUserRequest;

/// Transition `AppState` into `ConversationMode::Query` for the given request.
pub(crate) fn handle_query_request(state: &mut AppState, req: Option<QueryUserRequest>) {
    let Some(r) = req else { return };
    let qs = QueryState::builder()
        .question(r.question)
        .choices(r.choices)
        .freeform(PromptText::new(""))
        .reply_tx(r.reply_tx)
        .build();
    state.interaction.mode = ConversationMode::Query(qs);
}

/// Resolve the user's answer from query state and send it on the oneshot channel.
pub(crate) fn handle_query_submit(state: &mut AppState) {
    const THINKING_LABEL: &str = "Thinking...";

    let Some(qs) = state.take_query_state() else {
        return;
    };
    let Some(answer) = resolve_query_answer(&qs) else {
        return;
    };
    let ts = current_timestamp_ms();
    state.push_user_input_line(OutputText::new(format!("> {}", answer)), ts);
    state.push_output_newline();
    state.push_output_newline();
    state.agent.thinking.label = THINKING_LABEL.into();
    let _ = qs.reply_tx.send(answer);
}

/// Derive the user's answer from the query state.
pub(crate) fn resolve_query_answer(qs: &QueryState) -> Option<OutputText> {
    let trimmed = qs.freeform.trim();
    if !trimmed.is_empty() {
        return numeric_choice(trimmed, &qs.choices)
            .map(|choice| OutputText::new(choice.as_str()))
            .or_else(|| Some(OutputText::new(trimmed)));
    }
    qs.selected
        .and_then(|i| qs.choices.get(i))
        .map(|choice| OutputText::new(choice.as_str()))
}

fn numeric_choice(s: &str, choices: &[ChoiceText]) -> Option<ChoiceText> {
    let n: usize = s.parse().ok()?;
    if n >= 1 && n <= choices.len() {
        choices.get(n - 1).cloned()
    } else {
        None
    }
}
