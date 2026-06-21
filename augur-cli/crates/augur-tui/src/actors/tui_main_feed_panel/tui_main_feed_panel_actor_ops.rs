//! Private helper operations for the TUI main-feed panel actor.

use crate::domain::tui_state::OutputLine;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};

/// Append a token string to the last line, or start a new plain line.
///
/// Newlines within `text` are split: the first segment is appended to the
/// current last line; each subsequent segment begins a new plain line.
pub(super) fn append_token(lines: &mut Vec<OutputLine>, text: OutputText) {
    let raw_text = text.as_str().to_owned();
    if raw_text.contains('\n') {
        for (index, part) in raw_text.split('\n').enumerate() {
            if index == 0 {
                append_text_to_last(lines, part);
            } else {
                lines.push(OutputLine::plain(OutputText::new(part.to_owned())));
            }
        }
    } else {
        append_text_to_last(lines, &raw_text);
    }
}

fn append_text_to_last(lines: &mut Vec<OutputLine>, text: &str) {
    if let Some(last) = lines.last_mut() {
        let combined = format!("{}{}", last.text.as_str(), text);
        last.text = OutputText::new(combined);
    } else {
        lines.push(OutputLine::plain(OutputText::new(text.to_owned())));
    }
}
