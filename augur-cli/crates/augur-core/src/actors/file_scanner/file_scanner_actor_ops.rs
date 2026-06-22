//! Private helper operations for the file-scanner actor.

use super::commands::FileScanCmd;
use augur_domain::domain::newtypes::{Count, IsPredicate, NumericNewtype};
use augur_domain::domain::string_newtypes::{FileDisplayName, FilePath, StringNewtype};
use augur_domain::domain::types::FileCompletion;
use tokio::sync::{mpsc, watch};

/// Split a user prefix into `(directory, filename_fragment)` at the last slash.
pub(super) fn split_prefix(prefix: &FilePath) -> (FilePath, FileDisplayName) {
    match prefix.as_str().rfind('/') {
        Some(idx) => (
            FilePath::new(&prefix.as_str()[..=idx]),
            FileDisplayName::new(&prefix.as_str()[idx + 1..]),
        ),
        None => (FilePath::new("."), FileDisplayName::new(prefix.as_str())),
    }
}

/// Build one completion entry when the directory entry matches the fragment.
pub(super) fn build_completion(
    entry: std::fs::DirEntry,
    dir: &FilePath,
    fragment: &FileDisplayName,
) -> Option<FileCompletion> {
    let name = entry.file_name();
    let display_name = name.to_str()?;
    if !display_name.starts_with(fragment.as_str()) {
        return None;
    }
    let path_str = if dir.as_str() == "." {
        display_name.to_owned()
    } else {
        format!("{}{}", dir.as_str(), display_name)
    };
    Some(FileCompletion {
        path: FilePath::new(path_str),
        display_name: FileDisplayName::new(display_name),
    })
}

/// Apply one scan command and return `true` when the run loop should stop.
pub(super) async fn apply_scan_command(
    cmd: FileScanCmd,
    results_tx: &watch::Sender<Vec<FileCompletion>>,
    max_results: Count,
) -> IsPredicate {
    match cmd {
        FileScanCmd::Shutdown => IsPredicate::yes(),
        FileScanCmd::Scan { prefix } => {
            let _ = results_tx.send(collect_scan_results(&prefix, max_results));
            IsPredicate::no()
        }
    }
}

/// Drive the command loop for the file-scanner actor task.
pub(super) async fn run_scan_loop(
    mut cmd_rx: mpsc::Receiver<FileScanCmd>,
    results_tx: watch::Sender<Vec<FileCompletion>>,
    max_results: Count,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        if bool::from(apply_scan_command(cmd, &results_tx, max_results).await) {
            break;
        }
    }
}

/// Collect and sort file completions for a prefix, capped to `max_results`.
pub(super) fn collect_scan_results(prefix: &FilePath, max_results: Count) -> Vec<FileCompletion> {
    let (dir, fragment) = split_prefix(prefix);
    let entries = match std::fs::read_dir(dir.as_str()) {
        Ok(entries) => entries,
        Err(_) => return vec![],
    };
    let mut results: Vec<FileCompletion> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| build_completion(entry, &dir, &fragment))
        .collect();
    results.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    results.truncate(max_results.inner());
    results
}
