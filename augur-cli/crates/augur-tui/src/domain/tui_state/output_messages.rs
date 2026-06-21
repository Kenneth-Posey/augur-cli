//! Output-line construction helpers for `AppState`.

use super::*;
use augur_domain::domain::newtypes::ScrollOffset;
use augur_domain::domain::string_newtypes::StringNewtype;

impl AppState {
    /// Push an error line to the output pane.
    pub fn push_error_line(&mut self, text: impl Into<OutputText>) {
        let meta = self.agent.pending_response.take();
        let last_has_content = self
            .output
            .lines
            .last()
            .map(|l| !l.text.as_str().is_empty())
            .unwrap_or(false);
        if last_has_content {
            self.output.lines.push(OutputLine::plain(""));
        }
        let header = meta
            .map(|m| LineHeader {
                timestamp: Some(m.ts),
                model_prefix: None,
            })
            .unwrap_or_default();
        let text = text.into();
        let mut parts = text.as_str().split('\n');
        if let Some(first) = parts.next() {
            self.output.lines.push(
                OutputLine::builder()
                    .text(OutputText::new(first))
                    .kind(LineKind::Error)
                    .header(header)
                    .build(),
            );
            for part in parts {
                self.output.lines.push(OutputLine::error(part));
            }
        }
    }

    /// Push a tool-call line to the output pane without touching `pending_response_ts`.
    pub fn push_tool_call_line(&mut self, text: OutputText) {
        let trailing_blank = self
            .output
            .lines
            .last()
            .map(|l| {
                !matches!(l.kind, LineKind::UserInput | LineKind::ToolCall)
                    && l.text.as_str().is_empty()
            })
            .unwrap_or(false);
        if trailing_blank {
            self.output.lines.pop();
        }
        for part in text.as_str().split('\n') {
            self.output.lines.push(OutputLine::tool_call(part));
        }
    }

    /// Push a model intent line to the output pane.
    pub fn push_intent_line(&mut self, text: OutputText) {
        for part in text.as_str().split('\n') {
            self.output.lines.push(OutputLine::plain(part));
        }
        self.output.lines.push(OutputLine::plain(""));
    }

    /// Push a sub-agent self-feedback line to the output pane.
    pub fn push_self_feedback_line(&mut self, text: impl Into<OutputText>) {
        self.output.lines.push(OutputLine::self_feedback(text));
    }

    /// Push a user-input line directly to the output pane.
    ///
    /// Auto-scrolls to bottom if user was already at bottom (scroll_offset == 0).
    /// Preserves user's scroll position if they have scrolled up.
    /// Resets `agent.is_turn_complete` so the next agent turn can append its
    /// closing blank lines correctly.
    pub fn push_user_input_line(&mut self, text: OutputText, timestamp: TimestampMs) {
        let previous_offset = self.output.scroll_offset.get();
        let was_at_bottom = previous_offset.inner() == 0;
        self.agent.is_turn_complete = false.into();
        self.output.lines.push(
            OutputLine::builder()
                .text(text)
                .kind(LineKind::UserInput)
                .header(LineHeader {
                    timestamp: Some(timestamp),
                    model_prefix: None,
                })
                .build(),
        );
        let new_offset = if was_at_bottom {
            ScrollOffset::of(0)
        } else {
            previous_offset
        };
        self.output.scroll_offset.set(new_offset);
        tracing::info!(
            was_at_bottom,
            previous_offset = previous_offset.inner(),
            new_offset = new_offset.inner(),
            "tui.output.push_user_input_line.scroll"
        );
    }

    /// Push a system-message line to the output pane with the current wall-clock timestamp.
    pub fn push_system_message(&mut self, text: impl Into<OutputText>) {
        let ts = current_timestamp_ms();
        let text = text.into();
        let last_has_content = self
            .output
            .lines
            .last()
            .map(|l| !l.text.as_str().is_empty())
            .unwrap_or(false);
        if last_has_content {
            self.output.lines.push(OutputLine::plain(""));
        }
        let mut parts = text.as_str().split('\n');
        if let Some(first) = parts.next() {
            self.output.lines.push(
                OutputLine::builder()
                    .text(OutputText::new(first))
                    .kind(LineKind::System)
                    .header(LineHeader {
                        timestamp: Some(ts),
                        model_prefix: None,
                    })
                    .build(),
            );
            for part in parts {
                self.output.lines.push(
                    OutputLine::builder()
                        .text(OutputText::new(part))
                        .kind(LineKind::System)
                        .header(LineHeader::default())
                        .build(),
                );
            }
        }
    }
}
