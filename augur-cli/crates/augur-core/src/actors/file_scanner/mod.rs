//! FileScannerActor: async filesystem path scanner for `@`-attachment autocomplete.
//!
//! Exposes `FileScannerHandle` for the TUI event loop and `parse_file_attachments`
//! for stripping `@path` tokens from a prompt string at submit time. Enables
//! efficient file path completion and attachment handling in interactive mode.

pub mod commands;
pub mod file_scanner_actor;
mod file_scanner_actor_ops;
pub mod handle;

pub use file_scanner_actor::spawn;
pub use handle::FileScannerHandle;

use augur_domain::domain::string_newtypes::{FilePath, PromptText, StringNewtype};

/// Parse `@path` attachment tokens out of a prompt string.
///
/// Splits `text` on ASCII whitespace. Tokens that start with `@` are
/// stripped of the leading `@` and collected as `FilePath` attachment values.
/// All remaining tokens are joined with a single space to form the clean
/// prompt. Returns `(clean_prompt, attachments)`.
///
/// An input consisting only of `@` tokens returns an empty `clean_prompt`.
/// Call site: `key_dispatch::handle_submit` `NotACommand` arm (Phase 4).
pub fn parse_file_attachments(text: &PromptText) -> (PromptText, Vec<FilePath>) {
    let mut clean_tokens: Vec<&str> = Vec::new();
    let mut attachments: Vec<FilePath> = Vec::new();
    for token in text.as_str().split_ascii_whitespace() {
        if let Some(path) = token.strip_prefix('@') {
            if !path.is_empty() {
                attachments.push(FilePath::new(path));
            }
        } else {
            clean_tokens.push(token);
        }
    }
    (PromptText::new(clean_tokens.join(" ")), attachments)
}
