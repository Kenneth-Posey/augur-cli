//! Asynchronous loader for instruction-prefix files injected into OpenRouter requests.
//!
//! Reads each listed file from disk (relative to a repo root) and builds an
//! [`InstructionPrefix`] containing one [`Message`] per successfully loaded file.
//! Files that cannot be read are skipped with a warning; no error is returned.

use augur_domain::task_types::{InstructionFilePath, InstructionPrefix, RepoRoot};
use augur_domain::types::Message;

/// Error produced when an instruction file cannot be decoded.
///
/// Currently defined for forward-compatibility. The loader skips unreadable
/// files rather than propagating IO errors; `Encoding` would be raised if a
/// file's bytes cannot be interpreted as valid UTF-8.
#[derive(Debug)]
pub enum InstructionLoadError {
    /// The file at `path` could not be decoded from UTF-8.
    Encoding {
        /// The path that failed to decode.
        path: InstructionFilePath,
        /// Human-readable description of the encoding error.
        source: String,
    },
}

impl std::fmt::Display for InstructionLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encoding { path, source } => {
                write!(f, "encoding error in '{}': {}", path, source)
            }
        }
    }
}

impl std::error::Error for InstructionLoadError {}

/// Load instruction files and return an [`InstructionPrefix`] for injection.
///
/// For each path in `paths`, the absolute location is constructed by joining
/// `repo_root` and the relative path. Files that fail to read (not found,
/// permission denied, etc.) emit a `tracing::warn!` and are silently skipped;
/// the returned prefix contains only the successfully loaded files.
///
/// When a file is not found at the repo-relative path, falls back to the
/// installed config directory (`~/.augur-cli/...`) so instruction, skill, and
/// prompt files placed there are also discovered.
///
/// # Inputs
/// - `paths`: slice of relative file paths to load, in order.
/// - `repo_root`: absolute path to the repository root used as the base.
///
/// # Outputs
/// Returns `Ok(InstructionPrefix)` containing one `User` message per loaded
/// file. The message text is `"[FILE: <path>]\n<file content>"`.
///
/// # Errors
/// Returns `Err(InstructionLoadError::Encoding)` only if a file is read
/// successfully but cannot be decoded as UTF-8. In practice the current
/// implementation uses `tokio::fs::read_to_string` which performs the decode,
/// so any IO or UTF-8 error is treated as a skip.
pub async fn load_instruction_prefix(
    paths: &[InstructionFilePath],
    repo_root: &RepoRoot,
) -> Result<InstructionPrefix, InstructionLoadError> {
    let mut messages = Vec::with_capacity(paths.len());
    for path in paths {
        let abs = format!("{}/{}", repo_root.0, path.0);
        match tokio::fs::read_to_string(&abs).await {
            Ok(content) => {
                let text = format!("[FILE: {}]\n{}", path.0, content);
                messages.push(Message::user(text));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // Fall back to installed config directory.
                if let Some(msg) = try_read_from_install(path).await {
                    messages.push(msg);
                } else {
                    tracing::warn!(
                        path = %path,
                        error = %err,
                        "instruction file not readable from repo or install; skipping"
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    path = %path,
                    error = %err,
                    "instruction file not readable; skipping"
                );
            }
        }
    }
    Ok(InstructionPrefix(messages))
}

/// Try to read an instruction file from `~/.augur-cli/{path}`.
async fn try_read_from_install(path: &InstructionFilePath) -> Option<Message> {
    let home = std::env::var("HOME").ok()?;
    let install_path = format!("{}/.augur-cli/{}", home, path.0);
    match tokio::fs::read_to_string(&install_path).await {
        Ok(content) => {
            let text = format!("[FILE: {}]\n{}", path.0, content);
            Some(Message::user(text))
        }
        Err(_) => None,
    }
}
