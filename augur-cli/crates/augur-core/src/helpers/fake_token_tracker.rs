//! Test helper: factory for a throwaway `TokenTrackerHandle` for use in tests.

use crate::actors::TokenTrackerHandle;
use crate::actors::token_tracker;

/// Spawn a minimal token-tracker actor and return its handle.
///
/// The actor is started in-memory and a temporary directory is intentionally
/// forgotten (leaked via `std::mem::forget`) so callers need not store the `TempDir`.
/// Use in tests that construct `AgentServices` or other structs requiring
/// a `TokenTrackerHandle` without caring about actual token accumulation.
pub fn fake_token_tracker_handle() -> (tokio::task::JoinHandle<()>, TokenTrackerHandle) {
    let tmp = tempfile::tempdir().expect("tempdir for fake token tracker");
    let result = token_tracker::token_tracker_actor::spawn();
    std::mem::forget(tmp);
    result
}
