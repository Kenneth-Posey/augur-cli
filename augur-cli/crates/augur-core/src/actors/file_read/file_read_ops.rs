//! Pure logic for file range extraction and allowed-directory checking.

pub use crate::tools::ports::is_within_allowed_dirs;
pub(crate) use crate::tools::ports::{FileReadResult, ReadRange};
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype};
use tokio::sync::oneshot;

/// Commands consumed by the file-read actor task loop.
pub enum FileReadCommand {
    /// Count the number of lines in the given file.
    LineCount {
        /// Path to the file to count.
        path: FilePath,
        /// Channel to send the result back on.
        reply_tx: oneshot::Sender<FileReadResult>,
    },
    /// Read a range of lines from the given file.
    ReadRange {
        /// Path to the file to read.
        path: FilePath,
        /// Which lines to include in the output.
        range: ReadRange,
        /// Channel to send the result back on.
        reply_tx: oneshot::Sender<FileReadResult>,
    },
    /// Gracefully stop the actor task loop.
    Shutdown,
}

/// Extract the requested lines from `content` according to `range`.
///
/// Line numbers are 1-indexed. Start and end values are clamped to the actual
/// line count so callers never receive a panic or empty-range error from
/// out-of-bounds input. Use this in ops tests and the actor dispatch path.
pub(super) fn apply_range(content: &OutputText, range: &ReadRange) -> OutputText {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let (start, end) = range_bounds(range, total);
    OutputText::new(lines[start..end].join("\n"))
}

/// Convert a `ReadRange` to a `(start, end)` half-open index pair clamped to `[0, total]`.
///
/// `start` is the 0-indexed first line to include; `end` is one past the last.
/// Callers pass this directly to a slice expression: `lines[start..end]`.
fn range_bounds(range: &ReadRange, total: usize) -> (usize, usize) {
    match range {
        ReadRange::Full => (0, total),
        ReadRange::From(s) => (s.saturating_sub(1).min(total), total),
        ReadRange::To(e) => (0, (*e).min(total)),
        ReadRange::Between(start, end) => {
            let low = (*start).min(*end);
            let high = (*start).max(*end);
            (low.saturating_sub(1).min(total), high.min(total))
        }
    }
}
