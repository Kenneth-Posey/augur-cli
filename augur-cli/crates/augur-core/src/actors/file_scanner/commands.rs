//! FileScanCmd: commands sent to the file-scanner actor.

use augur_domain::domain::string_newtypes::FilePath;

/// Commands accepted by the file-scanner actor.
///
/// Sent exclusively through `FileScannerHandle`. The actor processes one
/// command at a time and publishes results on a shared watch channel.
pub enum FileScanCmd {
    /// Scan the filesystem for paths matching `prefix` and publish results.
    ///
    /// The prefix is the text the user has typed after the `@` character.
    /// The actor splits it into a directory and a filename fragment, reads
    /// that directory, and returns entries whose names start with the fragment.
    /// Sent by `FileScannerHandle::scan` on each TUI keypress.
    Scan { prefix: FilePath },

    /// Terminate the actor task loop gracefully.
    ///
    /// Sent by `FileScannerHandle::shutdown` during application shutdown.
    Shutdown,
}
