//! File-read actor module.
//!
//! Provides the leaf actor responsible for allowed-directory checks, line-count
//! requests, and line-range reads. Enforces file access permissions and project
//! boundaries, ensuring the agent can only read files within allowed directories.

/// Actor task that owns file-read request processing.
pub mod file_read_actor;
/// Private helper operations for the file-read actor.
mod file_read_actor_ops;
/// Pure file-read command and range types.
pub mod file_read_ops;
/// Public handle for issuing file-read requests.
pub mod handle;

pub use handle::FileReadHandle;
