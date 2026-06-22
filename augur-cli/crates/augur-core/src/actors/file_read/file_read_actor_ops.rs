//! Private helper operations for the file-read actor.

use super::file_read_ops::{apply_range, is_within_allowed_dirs, FileReadCommand};
use crate::tools::ports::{FileReadResult, ReadRange};
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Canonicalize and retain only allowed-directory entries that resolve successfully.
pub(super) fn canonicalize_dirs(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter()
        .filter_map(|d| match d.canonicalize() {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!(dir = %d.display(), error = %e, "allowed dir could not be canonicalized; skipping");
                None
            }
        })
        .collect()
}

/// Run the file-read command loop until shutdown.
pub(super) async fn run(allowed_dirs: Vec<PathBuf>, mut rx: mpsc::Receiver<FileReadCommand>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            FileReadCommand::Shutdown => break,
            FileReadCommand::LineCount { path, reply_tx } => {
                let result = handle_line_count(&path, &allowed_dirs).await;
                let _ = reply_tx.send(result);
            }
            FileReadCommand::ReadRange {
                path,
                range,
                reply_tx,
            } => {
                let result = handle_read_range(&path, range, &allowed_dirs).await;
                let _ = reply_tx.send(result);
            }
        }
    }
}

async fn handle_line_count(path: &FilePath, allowed_dirs: &[PathBuf]) -> FileReadResult {
    match resolve_allowed_path(Path::new(path.as_str()), allowed_dirs) {
        Err(msg) => error_result(msg),
        Ok(canonical) => match tokio::fs::read_to_string(&canonical).await {
            Err(e) => error_result(e.to_string()),
            Ok(content) => FileReadResult {
                output: OutputText::new(content.lines().count().to_string()),
                is_error: IsPredicate::from(false),
            },
        },
    }
}

async fn handle_read_range(
    path: &FilePath,
    range: ReadRange,
    allowed_dirs: &[PathBuf],
) -> FileReadResult {
    match resolve_allowed_path(Path::new(path.as_str()), allowed_dirs) {
        Err(msg) => error_result(msg),
        Ok(canonical) => match tokio::fs::read_to_string(&canonical).await {
            Err(e) => error_result(e.to_string()),
            Ok(content) => FileReadResult {
                output: apply_range(&OutputText::new(content), &range),
                is_error: IsPredicate::from(false),
            },
        },
    }
}

/// Canonicalize `path` and verify it is within one of the `allowed_dirs`.
fn resolve_allowed_path(path: &Path, allowed_dirs: &[PathBuf]) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| format!("cannot access path: {e}"))?;
    match is_within_allowed_dirs(&canonical, allowed_dirs) {
        Some(_) => Ok(canonical),
        None => Err("access denied: path is outside allowed directories".to_owned()),
    }
}

fn error_result(msg: String) -> FileReadResult {
    FileReadResult {
        output: OutputText::new(msg),
        is_error: IsPredicate::from(true),
    }
}
