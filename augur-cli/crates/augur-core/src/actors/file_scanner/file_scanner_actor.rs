//! FileScannerActor: async directory-scan actor with watch-channel publication.

use super::file_scanner_actor_ops as actor_ops;
use super::handle::FileScannerHandle;
use augur_domain::domain::channels::FILE_SCAN_COMMAND_CAPACITY;
use augur_domain::domain::newtypes::Count;
use augur_domain::domain::string_newtypes::FilePath;
use augur_domain::domain::types::FileCompletion;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

/// Maximum number of file completions returned from a single scan.
///
/// Caps the hint list height so the TUI layout is not overwhelmed by large
/// directories. Consumers: `scan_directory`, `render_file_hints`.
const MAX_SCAN_RESULTS: usize = 20;

/// Spawn the file-scanner actor and return a `FileScannerHandle`.
///
/// Creates the mpsc command channel and a watch channel initialised with an
/// empty result list. Spawns the actor task and returns a handle the TUI can
/// call `scan()` and `latest()` on without awaiting.
pub fn spawn() -> (JoinHandle<()>, FileScannerHandle) {
    let (cmd_tx, cmd_rx) = mpsc::channel(*FILE_SCAN_COMMAND_CAPACITY);
    let (results_tx, results_rx) = watch::channel(Vec::new());
    let handle = FileScannerHandle::new(cmd_tx, results_rx);
    let join = tokio::spawn(actor_ops::run_scan_loop(
        cmd_rx,
        results_tx,
        Count::of(MAX_SCAN_RESULTS),
    ));
    (join, handle)
}

/// Scan the filesystem for entries matching `prefix` and return completions.
///
/// Splits `prefix` at the last `/` to get `(dir, fragment)`. If no `/` is
/// present, scans `"."` filtering by `fragment = prefix`. Reads the directory
/// synchronously (acceptable at the ~1 scan/keypress rate). Returns at most
/// `MAX_SCAN_RESULTS` entries sorted by `display_name`. Returns an empty vec
/// on any I/O error.
pub fn scan_directory(prefix: &FilePath) -> Vec<FileCompletion> {
    actor_ops::collect_scan_results(prefix, Count::of(MAX_SCAN_RESULTS))
}
