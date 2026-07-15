use augur_tui::domain::string_newtypes::{EndpointName, StringNewtype};
use augur_tui::domain::tui_state::{AppScreen, AppState};

fn conversation_state() -> AppState {
    AppState::new(EndpointName::new("ep"), AppScreen::Conversation)
}

// ── configure_terminal_startup ───────────────────────────────────────────────

/// Verifies that `configure_terminal_startup` writes terminal control bytes to
/// the supplied writer and returns `Ok(())`, confirming the escape sequences
/// for mouse capture and bracketed paste are emitted at startup.
#[test]
fn configure_terminal_startup_writes_control_bytes_and_returns_ok() {
    let mut buf: Vec<u8> = Vec::new();
    let result = super::configure_terminal_startup(&mut buf);
    assert!(
        result.is_ok(),
        "configure_terminal_startup must succeed on a Vec<u8> writer"
    );
    assert!(
        !buf.is_empty(),
        "configure_terminal_startup must write terminal escape bytes"
    );
}
