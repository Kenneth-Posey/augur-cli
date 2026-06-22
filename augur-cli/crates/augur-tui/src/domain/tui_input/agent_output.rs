//! Agent-output application helpers for TUI input handling.

use super::*;
use augur_domain::domain::newtypes::WaitSecs;
use augur_domain::domain::string_newtypes::{ModelId, OutputText, ToolName};
use augur_domain::domain::tool_call_formatting::format_tool_call_line;

/// Append an optional terminal label and close out the current assistant turn.
pub fn push_turn_end(state: &mut AppState, label: Option<OutputText>) {
    if let Some(text) = label {
        state.push_output_newline();
        state.push_output_token(text);
    }
    state.push_output_newline();
    state.push_output_newline();
    state.agent.thinking.is_active = false.into();
    state.status.context_window.backoff_until = None;
}

/// Apply an agent-output event to the conversation state.
pub fn apply_agent_output(state: &mut AppState, output: AgentOutput) {
    let Some(output) = handle_turn_output(state, output) else {
        return;
    };
    let Some(output) = handle_tooling_output(state, output) else {
        return;
    };
    handle_status_output(state, output);
}

fn handle_token_output(state: &mut AppState, text: OutputText) {
    state.agent.thinking.label = "Thinking...".into();
    // Re-arm the idempotency guard on the first token of a new turn so that
    // background-agent turns (which have no preceding user-input line) also
    // receive their closing blank lines when Done / TurnComplete fires.
    state.agent.is_turn_complete = false.into();
    state.push_output_token(text);
}

fn finish_turn_output(state: &mut AppState) {
    if state.agent.is_turn_complete.into() {
        return;
    }
    state.agent.is_turn_complete = true.into();
    push_turn_end(state, None);
    refresh_status_bar_base_fields(&mut state.status);
}

fn handle_message_break(state: &mut AppState) {
    state.push_output_newline();
    state.push_output_newline();
    state.agent.pending_response = Some(
        PendingResponseMeta::builder()
            .ts(current_timestamp_ms())
            .model(state.status.model_display.clone())
            .build(),
    );
}

fn handle_error_output(state: &mut AppState, error: OutputText) {
    state.push_error_line(format!("[error] {error}"));
    push_turn_end(state, None);
    state.agent.is_turn_complete = true.into();
}

fn handle_interrupted_output(state: &mut AppState) {
    if state.agent.thinking.is_active.into() {
        push_turn_end(state, Some(OutputText::from("[stopped]")));
        state.agent.is_turn_complete = true.into();
    }
}

fn handle_tool_call_started(state: &mut AppState, name: ToolName, args: serde_json::Value) {
    let summary = format_tool_call_line(name.clone(), &args);

    // Remove trailing blank line if present (same logic as push_tool_call_line)
    let trailing_blank = state
        .output
        .lines
        .last()
        .map(|l| {
            !matches!(l.kind, LineKind::UserInput | LineKind::ToolCall)
                && l.text.as_str().is_empty()
        })
        .unwrap_or(false);
    if trailing_blank {
        state.output.lines.pop();
    }

    let mut summary_lines = summary.as_str().split('\n');
    if let Some(first_line) = summary_lines.next() {
        let line = crate::domain::tui_state::OutputLine::tool_call_with_metadata(
            OutputText::new(first_line),
            name.clone(),
            args.clone(),
        );
        state.output.lines.push(line);
    }
    for line in summary_lines {
        state
            .output
            .lines
            .push(crate::domain::tui_state::OutputLine::tool_call(line));
    }

    state.agent.thinking.label = format!("Calling {}...", name.as_str()).into();
}

fn push_system_message_line(state: &mut AppState, text: OutputText) {
    state.push_system_message(text);
    state.push_output_newline();
}

fn handle_compaction_complete(state: &mut AppState, text: OutputText) {
    push_system_message_line(state, text);
}

fn handle_tool_progress(state: &mut AppState, message: OutputText) {
    let line = format!("    \u{21bb} {}", message.as_str());
    state.push_tool_call_line(OutputText::new(line));
}

fn handle_tool_partial_result(state: &mut AppState, output: OutputText) {
    for part in output.as_str().split('\n') {
        state.push_self_feedback_line(part);
    }
}

fn handle_active_model_changed(state: &mut AppState, name: ModelId) {
    state.status.model_display = if name.is_empty() {
        "auto".into()
    } else {
        name.to_string().into()
    };
    state.prompt.models.active_id = Some(name);
}

fn handle_backoff_started(state: &mut AppState, wait: WaitSecs) {
    let deadline = Instant::now() + std::time::Duration::from_secs(wait.inner());
    state.status.context_window.backoff_until = Some(deadline);
}

fn handle_turn_output(state: &mut AppState, output: AgentOutput) -> Option<AgentOutput> {
    if apply_turn_output(state, &output) {
        None
    } else {
        Some(output)
    }
}

fn apply_turn_output(state: &mut AppState, output: &AgentOutput) -> bool {
    apply_turn_primary_output(state, output) || apply_turn_secondary_output(state, output)
}

fn apply_turn_primary_output(state: &mut AppState, output: &AgentOutput) -> bool {
    match output {
        AgentOutput::Token(text) => {
            handle_token_output(state, text.clone());
            true
        }
        AgentOutput::Done | AgentOutput::TurnComplete => {
            finish_turn_output(state);
            true
        }
        AgentOutput::MessageBreak => {
            handle_message_break(state);
            true
        }
        _ => false,
    }
}

fn apply_turn_secondary_output(state: &mut AppState, output: &AgentOutput) -> bool {
    match output {
        AgentOutput::Error(error) => {
            handle_error_output(state, error.clone());
            true
        }
        AgentOutput::Interrupted => {
            handle_interrupted_output(state);
            true
        }
        _ => false,
    }
}

fn handle_tooling_output(state: &mut AppState, output: AgentOutput) -> Option<AgentOutput> {
    match output {
        AgentOutput::ToolCallStarted { name, args } => handle_tool_call_started(state, name, args),
        AgentOutput::ToolProgress { message, .. } => handle_tool_progress(state, message),
        AgentOutput::ToolPartialResult { output, .. } => handle_tool_partial_result(state, output),
        _ => return Some(output),
    }
    None
}

fn handle_status_output(state: &mut AppState, output: AgentOutput) {
    let Some(output) = handle_usage_status_output(state, output) else {
        return;
    };
    handle_display_status_output(state, output);
}

fn handle_usage_status_output(state: &mut AppState, output: AgentOutput) -> Option<AgentOutput> {
    match output {
        AgentOutput::UsageUpdate { model } => {
            if let Some(m) = model {
                state.status.model_display = m.as_str().into();
                state.prompt.models.active_id = Some(m);
            }
        }
        AgentOutput::UsageSnapshot(totals) => {
            state.status.token_totals = totals;
        }
        _ => return Some(output),
    }
    None
}

fn handle_display_status_output(state: &mut AppState, output: AgentOutput) {
    match handle_primary_status_output(state, output) {
        Ok(()) => {}
        Err(output) => handle_secondary_status_output(state, output),
    }
}

fn handle_primary_status_output(
    state: &mut AppState,
    output: AgentOutput,
) -> Result<(), AgentOutput> {
    match output {
        AgentOutput::SystemMessage(text) => {
            push_system_message_line(state, text);
            Ok(())
        }
        AgentOutput::CompactionComplete { text } => {
            handle_compaction_complete(state, text);
            Ok(())
        }
        AgentOutput::IntentMessage(text) => {
            state.push_intent_line(text);
            Ok(())
        }
        _ => Err(output),
    }
}

fn handle_secondary_status_output(state: &mut AppState, output: AgentOutput) {
    if let AgentOutput::ModelsAvailable(models) = output {
        if should_apply_models_available(state) {
            state.prompt.models.available = models;
        }
        return;
    }
    if let AgentOutput::ActiveModelChanged(name) = output {
        handle_active_model_changed(state, name);
        return;
    }
    if let AgentOutput::BackoffStarted(wait) = output {
        handle_backoff_started(state, wait);
    }
}

fn should_apply_models_available(state: &AppState) -> bool {
    let active_endpoint = &state.agent.endpoint_name;
    let row = state
        .prompt
        .models
        .endpoint_catalog
        .iter()
        .find(|row| &row.endpoint_name == active_endpoint);
    match row {
        Some(row) => row.supports_auto.into(),
        None => state.prompt.models.endpoint_catalog.is_empty(),
    }
}
