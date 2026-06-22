//! Output-token append helpers for `AppState`.

use super::*;
use augur_domain::domain::newtypes::ScrollOffset;
use augur_domain::domain::string_newtypes::StringNewtype;

impl AppState {
    /// Append a token to the output, splitting on newlines.
    ///
    /// Auto-scrolls to bottom if user was already at bottom (scroll_offset == 0).
    /// Preserves user's scroll position if they have scrolled up.
    pub fn push_output_token(&mut self, token: OutputText) {
        let previous_offset = self.output.scroll_offset.get();
        let was_at_bottom = previous_offset.inner() == 0;
        let text = token.as_str().to_owned();
        let contains_newline = text.contains('\n');
        if contains_newline {
            self.push_token_with_newlines(&text);
        } else {
            self.append_to_last_line(text);
        }
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
            contains_newline,
            "tui.output.push_output_token.scroll"
        );
    }

    /// Push an empty line to the output pane.
    ///
    /// Auto-scrolls to bottom if user was already at bottom (scroll_offset == 0).
    /// Preserves user's scroll position if they have scrolled up.
    pub fn push_output_newline(&mut self) {
        let previous_offset = self.output.scroll_offset.get();
        let was_at_bottom = previous_offset.inner() == 0;
        self.output.lines.push(OutputLine::plain(""));
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
            "tui.output.push_output_newline.scroll"
        );
    }

    fn push_token_with_newlines(&mut self, text: &str) {
        let parts: Vec<&str> = text.split('\n').collect();
        for (i, part) in parts.iter().enumerate() {
            if i == 0 {
                self.append_to_last_line((*part).to_owned());
            } else {
                self.output.lines.push(OutputLine::plain(*part));
            }
        }
    }

    fn append_to_last_line(&mut self, text: String) {
        let meta = self.agent.pending_response.take();
        let last_prevents_append = last_line_prevents_append(self.output.lines.last());
        if !last_prevents_append && self.try_append_existing_line(&text, meta.clone()) {
            return;
        }
        if last_prevents_append {
            self.output.lines.push(OutputLine::plain(""));
        }
        let header = build_header_from_pending_response(meta);
        self.output.lines.push(
            OutputLine::builder()
                .text(OutputText::new(text))
                .kind(LineKind::Plain)
                .header(header)
                .build(),
        );
    }

    fn try_append_existing_line(
        &mut self,
        text: &str,
        meta: Option<crate::domain::tui_state::PendingResponseMeta>,
    ) -> bool {
        let Some(last) = self.output.lines.last_mut() else {
            return false;
        };
        if last.header.timestamp.is_none()
            && let Some(m) = meta
        {
            last.header = build_header_from_pending_response(Some(m));
        }
        let combined = format!("{}{}", last.text.as_str(), text);
        last.text = OutputText::new(combined);
        true
    }
}

fn last_line_prevents_append(last: Option<&OutputLine>) -> bool {
    last.map(|l| {
        matches!(
            l.kind,
            LineKind::ToolCall
                | LineKind::Error
                | LineKind::SelfFeedback
                | LineKind::UserInput
                | LineKind::System
        )
    })
    .unwrap_or(false)
}

fn build_header_from_pending_response(
    meta: Option<crate::domain::tui_state::PendingResponseMeta>,
) -> LineHeader {
    meta.map(|m| LineHeader {
        timestamp: Some(m.ts),
        model_prefix: (!m.model.is_empty()).then_some(m.model),
    })
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "../../../tests/domain/tui_state/output_flow.tests.rs"]
mod tests;
