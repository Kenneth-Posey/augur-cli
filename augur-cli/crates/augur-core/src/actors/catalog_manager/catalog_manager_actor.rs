//! Catalog manager actor shell.

use super::catalog_manager_actor_ops as actor_ops;
use super::handle::CatalogManagerHandle;

/// Spawns the catalog manager actor and returns its handle.
///
/// The actor listens for catalog generation requests and processes them
/// sequentially. It does not maintain any persistent state between requests.
pub fn spawn() -> CatalogManagerHandle {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(actor_ops::run_actor(rx));
    CatalogManagerHandle::new(tx)
}
