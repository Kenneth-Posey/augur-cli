//! Test helper: factory for a throwaway `LoggerHandle` for use in TUI handle tests.

use crate::actors::LoggerHandle;
use crate::actors::logger::logger_actor::spawn as spawn_logger;

/// Spawn a minimal logger actor and return its handle.
///
/// The actor writes to a temporary directory that is intentionally forgotten
/// (leaked via `std::mem::forget`) so callers need not store the `TempDir`.
/// Use in tests that construct `TuiToolHandles` and need a `LoggerHandle`
/// without caring about the actual log output.
pub fn fake_logger_handle() -> (tokio::task::JoinHandle<()>, LoggerHandle) {
    let log_tmp = tempfile::tempdir().expect("log tempdir for fake logger");
    let result = spawn_logger(log_tmp.path().to_path_buf());
    std::mem::forget(log_tmp);
    result
}
