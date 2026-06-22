//! Shared SSE streaming helpers for provider crates.

use augur_domain::domain::string_newtypes::{AccumulatedText, OutputText, StringNewtype};

/// Borrowed SSE byte chunk wrapper for shared provider parsing.
#[derive(Clone, Copy, Debug)]
pub struct SseChunk<'a>(pub &'a [u8]);

impl<'a> From<&'a [u8]> for SseChunk<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self(value)
    }
}

/// Drain complete SSE lines from a carry buffer plus a new byte chunk.
///
/// Appends the lossy UTF-8 decoding of `bytes` to `carry`, returns all
/// newline-terminated non-empty lines, and retains any trailing partial line in
/// `carry` for the next chunk. Used by streaming providers so a split `data:`
/// line is not dropped when the HTTP body arrives mid-line.
pub fn drain_complete_sse_lines(
    carry: &mut AccumulatedText,
    bytes: SseChunk<'_>,
) -> Vec<OutputText> {
    let mut next = carry.as_str().to_owned();
    next.push_str(&String::from_utf8_lossy(bytes.0));
    let mut parts: Vec<&str> = next.split('\n').collect();
    let remainder = parts.pop().unwrap_or_default().to_owned();
    let lines = parts
        .into_iter()
        .filter(|line| !line.is_empty())
        .map(OutputText::from)
        .collect();
    *carry = AccumulatedText::from(remainder);
    lines
}
