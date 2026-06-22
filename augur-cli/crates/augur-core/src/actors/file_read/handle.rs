//! FileReadHandle: cloneable client for the file-read actor.

use crate::tools::ports::{FileReadPort, FileReadResult, ReadRange};
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype};
use tokio::sync::{mpsc, oneshot};

use super::file_read_ops::FileReadCommand;

/// Cloneable client handle to the running file-read actor.
///
/// Wraps the mpsc command sender. Cloning shares the same actor task - both
/// `FileReadRangeTool` and `FileLineCountTool` hold a clone of this handle.
/// Dropping all clones causes the actor to drain its queue and exit.
#[derive(Clone)]
pub struct FileReadHandle {
    tx: mpsc::Sender<FileReadCommand>,
}

impl FileReadHandle {
    /// Create a new handle around the command sender. Called only by `spawn`.
    pub(super) fn new(tx: mpsc::Sender<FileReadCommand>) -> Self {
        FileReadHandle { tx }
    }

    /// Send a graceful shutdown signal to the file-read actor.
    pub fn shutdown(&self) {
        let _ = self.tx.try_send(FileReadCommand::Shutdown);
    }

    /// Request the number of lines in `path`.
    ///
    /// Returns a `FileReadResult` whose `output` is the decimal line count on
    /// success, or an error message on I/O failure or access-denied conditions.
    /// Returns an error result if the actor task has stopped.
    #[tracing::instrument(skip(self), fields(path = %path))]
    pub async fn line_count(&self, path: FilePath) -> FileReadResult {
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = FileReadCommand::LineCount { path, reply_tx };
        if self.tx.send(cmd).await.is_err() {
            return actor_stopped_result();
        }
        reply_rx.await.unwrap_or_else(|_| actor_dropped_result())
    }

    /// Request a range of lines from `path`.
    ///
    /// Returns a `FileReadResult` whose `output` contains the requested lines
    /// joined by `\n`. Returns an error result on I/O failure, access-denied,
    /// or if the actor task has stopped.
    #[tracing::instrument(skip(self), fields(path = %path))]
    pub async fn read_range(&self, path: FilePath, range: ReadRange) -> FileReadResult {
        let (reply_tx, reply_rx) = oneshot::channel();
        let cmd = FileReadCommand::ReadRange {
            path,
            range,
            reply_tx,
        };
        if self.tx.send(cmd).await.is_err() {
            return actor_stopped_result();
        }
        reply_rx.await.unwrap_or_else(|_| actor_dropped_result())
    }
}

#[async_trait::async_trait]
impl FileReadPort for FileReadHandle {
    async fn line_count(&self, path: FilePath) -> FileReadResult {
        FileReadHandle::line_count(self, path).await
    }

    async fn read_range(&self, path: FilePath, range: ReadRange) -> FileReadResult {
        FileReadHandle::read_range(self, path, range).await
    }
}

fn actor_stopped_result() -> FileReadResult {
    FileReadResult {
        output: OutputText::new("file read actor stopped"),
        is_error: IsPredicate::from(true),
    }
}

fn actor_dropped_result() -> FileReadResult {
    FileReadResult {
        output: OutputText::new("file read actor dropped reply"),
        is_error: IsPredicate::from(true),
    }
}
