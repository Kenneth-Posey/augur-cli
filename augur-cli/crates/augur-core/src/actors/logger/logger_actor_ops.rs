//! Private helper operations for the logger actor run loop.

use crate::actors::logger::logger_ops::{
    format_as_jsonl, history_entry_to_log_entry, message_log_file_name, message_to_entry,
    LogCommand, LogEntry,
};
use augur_domain::domain::newtypes::{NumericNewtype, TimestampMs, TimestampSecs};
use augur_domain::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use augur_domain::domain::types::Message;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// Record a write failure via tracing while ignoring successful writes.
pub(super) fn log_write_result(result: anyhow::Result<()>, log_path: &Path, message: &OutputText) {
    if let Err(error) = result {
        tracing::warn!(path = %log_path.display(), error = %error, "{}", message.as_str());
    }
}

/// Create the log directory and return the session JSONL path.
pub(super) async fn prepare_log_file(
    log_dir: &PathBuf,
    session_secs: TimestampSecs,
) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(log_dir).await?;
    Ok(log_dir.join(message_log_file_name(session_secs)))
}

/// Build newline-delimited JSONL payload for a message batch.
pub(super) fn build_messages_payload(messages: &[Message], endpoint: &EndpointName) -> OutputText {
    let mut payload = String::new();
    for message in messages {
        let entry = message_to_entry(message, endpoint);
        payload.push_str(&format_as_jsonl(&entry));
        payload.push('\n');
    }
    OutputText::new(payload)
}

/// Append payload bytes to a log file and sync data to disk.
pub(super) async fn append_payload(log_path: &PathBuf, payload: &OutputText) -> anyhow::Result<()> {
    let mut file = open_append(log_path).await?;
    file.write_all(payload.as_str().as_bytes()).await?;
    file.sync_data().await?;
    Ok(())
}

/// Drive the logger command loop until `Shutdown` or channel close.
pub(super) async fn run_command_loop(log_path: &PathBuf, rx: &mut mpsc::Receiver<LogCommand>) {
    while let Some(cmd) = rx.recv().await {
        if !handle_log_command(log_path, cmd).await {
            break;
        }
    }
}

/// Drain logger commands after fatal setup failure so senders can close cleanly.
pub(super) async fn drain(mut rx: mpsc::Receiver<LogCommand>) {
    while let Some(_cmd) = rx.recv().await {}
}

async fn handle_log_command(log_path: &PathBuf, cmd: LogCommand) -> bool {
    match cmd {
        LogCommand::Shutdown => false,
        LogCommand::LogMessages { endpoint, messages } => {
            log_write_result(
                append_messages(log_path, &endpoint, &messages).await,
                log_path,
                &OutputText::new("failed to write log entries"),
            );
            true
        }
        LogCommand::LogLine { role, content } => {
            log_write_result(
                append_single_entry(log_path, build_tui_entry(role, content)).await,
                log_path,
                &OutputText::new("failed to write log line"),
            );
            true
        }
        LogCommand::LogHistoryEntry(entry) => {
            let endpoint = EndpointName::new("history".to_owned());
            let log_entry = history_entry_to_log_entry(&entry, &endpoint);
            log_write_result(
                append_single_entry(log_path, log_entry).await,
                log_path,
                &OutputText::new("failed to write history entry"),
            );
            true
        }
        LogCommand::LogLlmRaw {
            direction,
            provider,
            model,
            body,
        } => {
            let ts_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let line = serde_json::json!({
                "ts": ts_ms,
                "role": "llm_raw",
                "endpoint": provider,
                "direction": direction,
                "model": model,
                "body": body,
            })
            .to_string();
            log_write_result(
                append_payload(
                    log_path,
                    &OutputText::new(format!("{line}\n")),
                )
                .await,
                log_path,
                &OutputText::new("failed to write llm_raw entry"),
            );
            true
        }
    }
}

async fn append_messages(
    log_path: &PathBuf,
    endpoint: &EndpointName,
    messages: &[Message],
) -> anyhow::Result<()> {
    let payload = build_messages_payload(messages, endpoint);
    append_payload(log_path, &payload).await
}

async fn append_single_entry(log_path: &Path, entry: LogEntry) -> anyhow::Result<()> {
    append_payload(
        &log_path.to_path_buf(),
        &OutputText::new(format!("{}\n", format_as_jsonl(&entry))),
    )
    .await
}

fn build_tui_entry(role: String, content: String) -> LogEntry {
    let ts = TimestampMs::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    );
    LogEntry {
        ts,
        role: role.into(),
        endpoint: "tui".to_string().into(),
        content: content.into(),
    }
}

async fn open_append(path: &PathBuf) -> anyhow::Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    Ok(file)
}
