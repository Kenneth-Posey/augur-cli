//! LoggerActor: writes all LLM conversation messages to a per-session JSONL file.

use super::handle::LoggerHandle;
use super::logger_actor_ops as actor_ops;
use super::logger_ops::{current_unix_secs, LogCommand};
use augur_domain::domain::channels::LOGGER_COMMAND_CAPACITY;
use augur_domain::domain::newtypes::TimestampSecs;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Spawn the logger actor and return its join handle and a communication handle.
///
/// Creates `log_dir` if it does not exist, then opens (or creates) the session
/// log file named `<unix_seconds>_msg.jsonl` inside that directory. The file is
/// opened in append mode so restarts within the same second extend the same log.
/// All logging I/O happens inside the actor task; callers never block on disk.
pub fn spawn(log_dir: PathBuf) -> (JoinHandle<()>, LoggerHandle) {
    let session_secs = current_unix_secs();
    spawn_with_session(log_dir, session_secs)
}

/// Spawn the logger actor using a precomputed session timestamp.
pub fn spawn_with_session(
    log_dir: PathBuf,
    session_secs: TimestampSecs,
) -> (JoinHandle<()>, LoggerHandle) {
    let (tx, rx) = mpsc::channel(*LOGGER_COMMAND_CAPACITY);
    let handle = LoggerHandle::new(tx);
    let join = tokio::spawn(run(log_dir, session_secs, rx));
    (join, handle)
}

async fn run(log_dir: PathBuf, session_secs: TimestampSecs, mut rx: mpsc::Receiver<LogCommand>) {
    let log_path = match actor_ops::prepare_log_file(&log_dir, session_secs).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(dir = %log_dir.display(), error = %e, "logger could not create log file; logging disabled");
            actor_ops::drain(rx).await;
            return;
        }
    };

    actor_ops::run_command_loop(&log_path, &mut rx).await;
}
