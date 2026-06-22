//! Test helper: factory for a throwaway `CatalogManagerHandle` for use in TUI handle tests.

use crate::actors::catalog_manager::CatalogManagerHandle;
use crate::actors::catalog_manager::catalog_manager_actor::spawn as spawn_catalog_manager;

/// Spawn a minimal catalog manager actor and return its handle.
///
/// Use in tests that construct `TuiHandles` and need a `CatalogManagerHandle`
/// without caring about the actual catalog generation output.
pub fn fake_catalog_manager_handle() -> (tokio::task::JoinHandle<()>, CatalogManagerHandle) {
    let handle = spawn_catalog_manager();
    // Create a dummy JoinHandle that completes immediately since spawn_catalog_manager
    // doesn't return one (the actor runs in the background)
    let dummy_join = tokio::spawn(async {});
    (dummy_join, handle)
}
