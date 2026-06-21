//! Output buffer helpers: token animation draining and agent output routing.

use crate::domain::tui_input::apply_agent_output;
use crate::domain::tui_state::AppState;
use augur_domain::domain::newtypes::Count;
use augur_domain::domain::string_newtypes::OutputText;
use augur_domain::domain::types::AgentOutput;
use std::ops::ControlFlow;
use tokio::sync::broadcast;

/// Status label text shown while the agent is processing a response.
const THINKING_LABEL: &str = "Thinking...";

/// Process one agent output event. Buffers Token text in `char_buf` for
/// smooth character-by-character animation; applies all other events immediately.
///
/// For non-Token events that end a turn (Done, Error, Interrupted), the char buffer
/// is flushed first so any remaining buffered text appears before the turn-end state.
///
/// When a `ModelsAvailable` event arrives and the model picker is already open
/// (buffer is "/model" or starts with "/model "), the picker list is refreshed
/// immediately so the user does not need to press a key to trigger the update.
/// Returns `true` on a closed channel.
pub(crate) fn handle_agent_output(
    state: &mut AppState,
    agent_out: Result<AgentOutput, broadcast::error::RecvError>,
    char_buf: &mut OutputText,
) -> ControlFlow<()> {
    match agent_out {
        Err(broadcast::error::RecvError::Closed) => ControlFlow::Break(()),
        Err(broadcast::error::RecvError::Lagged(n)) => {
            // CONFIRMED: structured tracing with `skipped` field for diagnostic clarity.
            tracing::warn!(skipped = n, "TUI lagged behind agent output");
            ControlFlow::Continue(())
        }
        Ok(AgentOutput::Token(t)) => {
            state.agent.thinking.label = THINKING_LABEL.into();
            char_buf.push_output(&t);
            ControlFlow::Continue(())
        }
        Ok(output) => {
            let is_models_available = matches!(output, AgentOutput::ModelsAvailable(_));
            flush_char_buf(state, char_buf);
            apply_agent_output(state, output);
            if is_models_available {
                refresh_model_picker_if_open(state);
            }
            ControlFlow::Continue(())
        }
    }
}

/// Drain all buffered agent output events from `output_rx` into `char_buf` without blocking.
///
/// Tokens are pushed to `char_buf` for animated display rather than applied
/// directly to state. Non-token events flush `char_buf` first (so buffered text
/// appears before any structural event), then are applied immediately. Stops on
/// an empty channel, a closed channel, or after a terminal event (Done, Error,
/// Interrupted) so those are reflected in the next render promptly.
///
/// Returns `true` if at least one event was drained, signalling to the caller
/// that visible state may have changed and a render should be issued.
///
/// Consumers: `run` in `actor.rs` after each `select_next_event` to drain
/// any remaining broadcast messages accumulated during the await.
pub(crate) fn drain_channel_to_buf(
    state: &mut AppState,
    output_rx: &mut broadcast::Receiver<AgentOutput>,
    char_buf: &mut OutputText,
) -> Option<()> {
    let mut drained = false;
    loop {
        match output_rx.try_recv() {
            Ok(AgentOutput::Token(t)) => {
                if !matches!(
                    state.interaction.screen,
                    crate::domain::tui_state::AppScreen::SessionSelector(_)
                ) {
                    char_buf.push_output(&t);
                    drained = true;
                }
            }
            Ok(output) => {
                if should_skip_picker_output(state, &output) {
                    continue;
                }
                let is_terminal = apply_drained_output_event(state, output, char_buf);
                drained = true;
                if is_terminal {
                    break;
                }
            }
            // CONFIRMED: structured tracing with `skipped` field for diagnostic clarity.
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "TUI lagged draining agent output");
            }
            Err(_) => break,
        }
    }
    drained.then_some(())
}

fn should_skip_picker_output(state: &AppState, output: &AgentOutput) -> bool {
    // State-initialisation events must be applied even in picker mode
    // so that model lists and context metrics are ready when the user
    // transitions to Chat. Streaming/chat-lifecycle events are skipped
    // while the picker is showing.
    matches!(
        state.interaction.screen,
        crate::domain::tui_state::AppScreen::SessionSelector(_)
    ) && !matches!(output, AgentOutput::ModelsAvailable(_))
}

fn apply_drained_output_event(
    state: &mut AppState,
    output: AgentOutput,
    char_buf: &mut OutputText,
) -> bool {
    let is_terminal = matches!(
        output,
        AgentOutput::Done
            | AgentOutput::Error(_)
            | AgentOutput::Interrupted
            | AgentOutput::TurnComplete
    );
    let is_complete = matches!(output, AgentOutput::Done | AgentOutput::TurnComplete);
    let is_models_available = matches!(output, AgentOutput::ModelsAvailable(_));
    flush_char_buf(state, char_buf);
    apply_agent_output(state, output);
    if is_models_available {
        refresh_model_picker_if_open(state);
    }
    if is_complete {
        ring_terminal_bell();
    }
    is_terminal
}

/// Emit the ASCII BEL character to the terminal.
///
/// Works in raw mode: terminals handle BEL independently of display rendering.
fn ring_terminal_bell() {
    use std::io::Write;
    let _ = std::io::stdout().write_all(b"\x07");
    let _ = std::io::stdout().flush();
}

/// Drain up to `n` characters from `char_buf` and push them to the output pane.
///
/// Splits at a Unicode scalar boundary so multi-byte characters are never
/// truncated mid-codepoint. Called on every ticker tick to produce a smooth
/// character-by-character animation effect in the output display.
///
/// Consumers: `select_next_event` ticker arm in `actor.rs`.
pub(crate) fn drain_char_buf(state: &mut AppState, char_buf: &mut OutputText, n: Count) {
    if char_buf.is_empty() {
        return;
    }
    let byte_end = char_buf.prefix_byte_end(n);
    let chunk = char_buf.drain_prefix(byte_end);
    state.push_output_token(chunk);
}

/// Flush all remaining chars in `char_buf` to the output pane in one shot.
///
/// Called before applying any non-Token `AgentOutput` event so buffered text
/// always appears before Done/Error/ToolCallStarted markers, and when a turn
/// ends so text is never left invisible in the buffer.
///
/// Consumers: `handle_agent_output` and `drain_channel_to_buf` in this module.
pub(crate) fn flush_char_buf(state: &mut AppState, char_buf: &mut OutputText) {
    if char_buf.is_empty() {
        return;
    }
    let chunk = char_buf.take_all();
    state.push_output_token(chunk);
}

/// Refresh the model picker immediately when it is already open.
///
/// Called after `ModelsAvailable` is applied so the picker list is populated
/// without requiring the user to press another key. Only fires when the buffer
/// is "/model" or starts with "/model " (model-picker mode).
///
/// Consumers: `handle_agent_output` in this module.
fn refresh_model_picker_if_open(state: &mut AppState) {
    let is_picker_open =
        state.prompt.buffer.starts_with("/model ") || state.prompt.buffer.as_str() == "/model";
    if is_picker_open {
        super::key_dispatch::refresh_model_hints(state);
    }
}
