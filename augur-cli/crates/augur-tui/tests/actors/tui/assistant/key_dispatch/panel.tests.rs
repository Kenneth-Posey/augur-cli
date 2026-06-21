use crate::actors::tui::assistant::key_dispatch::panel::*;
use crate::domain::string_newtypes::EndpointName;
use crate::domain::tui_state::{AppScreen, AppState, InputFocus, SecondaryView};

fn conversation_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

/// Verifies that Ctrl+T closes the agent feed secondary view when it is already open.
#[test]
fn ctrl_t_closes_agent_feed_secondary_view() {
    let mut state = conversation_state();
    state.interaction.panel.secondary_view = Some(SecondaryView::AgentFeed);
    state.interaction.panel.input_focus = InputFocus::Main;

    toggle_agent_feed_view(&mut state);

    assert_eq!(state.interaction.panel.secondary_view, None);
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Main);
}

/// Verifies that Ctrl+T opens the agent feed secondary view when it is closed.
#[test]
fn ctrl_t_opens_agent_feed_secondary_view() {
    let mut state = conversation_state();
    state.interaction.panel.secondary_view = None;
    state.interaction.panel.input_focus = InputFocus::Main;

    toggle_agent_feed_view(&mut state);

    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::AgentFeed)
    );
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Main);
}

/// Verifies that Ctrl+T switches Ask to AgentFeed and resets focus back to Main.
#[test]
fn ctrl_t_switches_ask_secondary_view_to_agent_feed_and_resets_focus() {
    let mut state = conversation_state();
    state.interaction.panel.secondary_view = Some(SecondaryView::Ask);
    state.interaction.panel.input_focus = InputFocus::Ask;

    toggle_agent_feed_view(&mut state);

    assert_eq!(
        state.interaction.panel.secondary_view,
        Some(SecondaryView::AgentFeed)
    );
    assert_eq!(state.interaction.panel.input_focus, InputFocus::Main);
}
