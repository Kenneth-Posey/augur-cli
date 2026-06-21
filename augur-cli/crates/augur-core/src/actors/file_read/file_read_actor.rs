//! FileReadActor: enforces allowed-directory access and dispatches file reads.

use super::file_read_actor_ops as actor_ops;
use super::handle::FileReadHandle;
use augur_domain::domain::channels::FILE_READ_COMMAND_CAPACITY;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Spawn the file-read actor and return its join handle and a communication handle.
///
/// `allowed_dirs` is the list of root directories the actor permits reads from.
/// Relative paths are canonicalized at spawn time so `./` becomes the absolute
/// working directory and path-traversal attempts are caught at request time.
/// Each directory that fails to canonicalize is silently skipped with a WARN log.
pub fn spawn(allowed_dirs: Vec<PathBuf>) -> (JoinHandle<()>, FileReadHandle) {
    let (tx, rx) = mpsc::channel(*FILE_READ_COMMAND_CAPACITY);
    let handle = FileReadHandle::new(tx);
    let canonical = actor_ops::canonicalize_dirs(&allowed_dirs);
    let join = tokio::spawn(actor_ops::run(canonical, rx));
    (join, handle)
}
