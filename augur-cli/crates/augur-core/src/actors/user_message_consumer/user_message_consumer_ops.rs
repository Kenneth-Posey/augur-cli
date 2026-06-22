//! User message consumer ops: pure input classification.
//!
//! `parse_user_input` classifies a raw text string into a [`UserFeedMessage`]
//! with a [`UserInputTag`] indicating whether it is a raw command or a slash command.

use augur_domain::domain::feeds::{UserFeedMessage, UserInputTag};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};

// ── UserMessageCmd ────────────────────────────────────────────────────────────

/// Commands accepted by the user message consumer actor.
///
/// `ProcessInput` delivers a raw string for classification and routing.
/// `Shutdown` signals the actor to exit its receive loop cleanly.
#[derive(Debug)]
pub enum UserMessageCmd {
    /// Deliver a raw user input string to be classified and routed.
    ProcessInput(String),
    /// Signal the actor to exit its receive loop.
    Shutdown,
}

// ── parse_user_input ──────────────────────────────────────────────────────────

/// Classify a raw text input into a [`UserFeedMessage`].
///
/// Inputs: `text` - the raw string entered by the user.
/// Outputs: a [`UserFeedMessage`] with [`UserInputTag::ParsedCommand`] when
/// `text` starts with `'/'`, or [`UserInputTag::RawCommand`] otherwise.
/// No side effects; this is a pure function.
pub(crate) fn parse_user_input(text: &OutputText) -> UserFeedMessage {
    let tag = if text.as_str().starts_with('/') {
        UserInputTag::ParsedCommand
    } else {
        UserInputTag::RawCommand
    };
    UserFeedMessage {
        tag,
        text: text.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that plain text input is classified as RawCommand.
    #[test]
    fn parse_raw_command() {
        let msg = parse_user_input(&OutputText::from("hello world"));
        assert_eq!(msg.tag, UserInputTag::RawCommand);
        assert_eq!(msg.text, "hello world");
    }

    /// Verifies that a slash-prefixed input is classified as ParsedCommand.
    #[test]
    fn parse_slash_command_detected() {
        let msg = parse_user_input(&OutputText::from("/run tests"));
        assert_eq!(msg.tag, UserInputTag::ParsedCommand);
        assert_eq!(msg.text, "/run tests");
    }

    /// Verifies that an empty string is classified as RawCommand.
    #[test]
    fn parse_empty_string() {
        let msg = parse_user_input(&OutputText::from(""));
        assert_eq!(msg.tag, UserInputTag::RawCommand);
    }
}
