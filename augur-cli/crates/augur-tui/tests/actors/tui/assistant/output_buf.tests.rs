use crate::domain::newtypes::{Count, NumericNewtype};
use crate::domain::string_newtypes::{EndpointName, ModelLabel, OutputText, StringNewtype};
use crate::domain::tui_state::{AppScreen, AppState};

fn model_option(id: &str, display_name: &str) -> crate::domain::types::ModelOption {
    crate::domain::types::ModelOption::builder()
        .id(crate::domain::string_newtypes::ModelId::new(id))
        .display_name(ModelLabel::new(display_name))
        .build()
}

/// Verifies that drain_char_buf moves exactly n characters from the buffer to
/// the output pane, leaving the remainder in the buffer.
#[test]
fn drain_char_buf_moves_n_chars_and_leaves_remainder() {
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut buf = OutputText::new("hello world");
    super::drain_char_buf(&mut state, &mut buf, Count::new(5));
    assert_eq!(buf.as_str(), " world");
    let output_text: String = state
        .output
        .lines
        .iter()
        .map(|l| l.text.as_str().to_owned())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        output_text.contains("hello"),
        "drained chars must appear in output, got: {output_text:?}"
    );
}

/// Verifies that drain_channel_to_buf returns true when at least one token was
/// available in the channel, confirming state may have changed and a render is needed.
#[tokio::test]
async fn drain_channel_to_buf_returns_true_when_events_present() {
    use crate::domain::types::AgentOutput;
    use tokio::sync::broadcast;
    let (tx, mut rx) = broadcast::channel::<AgentOutput>(16);
    tx.send(AgentOutput::Token(OutputText::new("hello")))
        .unwrap();
    drop(tx);
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut char_buf = OutputText::new("");
    let drained = super::drain_channel_to_buf(&mut state, &mut rx, &mut char_buf);
    assert!(
        drained.is_some(),
        "drain_channel_to_buf must return true when tokens were present"
    );
    assert_eq!(
        char_buf.as_str(),
        "hello",
        "token text must be placed into char_buf"
    );
}

/// Verifies that drain_channel_to_buf returns false when the channel has no
/// messages, indicating no state change and allowing the render to be skipped.
#[test]
fn drain_channel_to_buf_returns_false_when_empty() {
    use crate::domain::types::AgentOutput;
    use tokio::sync::broadcast;
    let (_tx, mut rx) = broadcast::channel::<AgentOutput>(16);
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    let mut char_buf = OutputText::new("");
    let drained = super::drain_channel_to_buf(&mut state, &mut rx, &mut char_buf);
    assert!(
        drained.is_none(),
        "drain_channel_to_buf must return false when channel was empty"
    );
}

/// Verifies that handle_agent_output refreshes the model picker when ModelsAvailable
/// arrives while the picker is already open (buffer starts with "/model ").
///
/// This tests the async timing fix: if the user types "/model " before models load,
/// the picker must populate once ModelsAvailable is received without waiting for
/// the next keypress.
#[tokio::test]
async fn handle_agent_output_models_available_refreshes_open_picker() {
    use crate::domain::types::AgentOutput;
    let mut state = AppState::new(EndpointName::new("ep"), AppScreen::Conversation);
    // Simulate user has already typed "/model " - picker is open but empty
    state.prompt.buffer = "/model ".to_owned();
    assert!(
        state.prompt.completions.model_picker.items.is_empty(),
        "picker must start empty before models arrive"
    );

    let models = vec![model_option("gpt-4o", "GPT-4o")];
    let mut char_buf = OutputText::new("");
    let closed = super::handle_agent_output(
        &mut state,
        Ok(AgentOutput::ModelsAvailable(models)),
        &mut char_buf,
    );

    assert!(
        matches!(closed, std::ops::ControlFlow::Continue(())),
        "channel must not be reported as closed"
    );
    assert!(
        !state.prompt.completions.model_picker.items.is_empty(),
        "model picker must be populated after ModelsAvailable arrives with picker open"
    );
}
