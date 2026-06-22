use augur_domain::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use augur_tui::domain::tui_state::{AppScreen, AppState, LineKind};

/// Verifies output-message helpers append expected line kinds and content.
#[test]
fn push_output_message_helpers_append_lines() {
    let mut state = AppState::new(EndpointName::from("copilot"), AppScreen::Conversation);

    state.push_error_line("error line");
    state.push_tool_call_line(OutputText::new("tool line"));
    state.push_intent_line(OutputText::new("intent line"));
    state.push_self_feedback_line("self line");
    state.push_system_message("system line");

    assert!(state
        .output
        .lines
        .iter()
        .any(|line| matches!(line.kind, LineKind::Error)));
    assert!(state
        .output
        .lines
        .iter()
        .any(|line| matches!(line.kind, LineKind::ToolCall)));
    assert!(state
        .output
        .lines
        .iter()
        .any(|line| matches!(line.kind, LineKind::SelfFeedback)));
    assert!(state
        .output
        .lines
        .iter()
        .any(|line| matches!(line.kind, LineKind::System)));
}
