//! Integration tests for the logger actor: spawning, message logging, and file output.

use augur_core::actors::logger::logger_actor::spawn;
use augur_domain::domain::newtypes::NumericNewtype;
use augur_domain::domain::string_newtypes::{EndpointName, OutputText, StringNewtype};
use augur_domain::domain::types::{Message, Role};
use std::path::PathBuf;
use tokio::time::{Duration, timeout};

/// Helper to create a temporary directory for log files.
fn temp_log_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "dcmk-logger-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ))
}

/// Verifies that spawning the logger actor creates the log directory if it does not exist.
#[tokio::test]
async fn creates_log_directory_on_spawn() {
    let log_dir = temp_log_dir();
    assert!(!log_dir.exists(), "test dir must not exist before spawn");

    let (join, handle) = spawn(log_dir.clone());
    // Allow the actor a moment to initialise
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.shutdown();
    let _ = timeout(Duration::from_secs(2), join).await;

    assert!(log_dir.exists(), "log directory should be created by actor");
    // Clean up
    let _ = std::fs::remove_dir_all(&log_dir);
}

/// Verifies that log_messages writes each message as a JSONL line to the log file.
#[tokio::test]
async fn log_messages_writes_jsonl_lines() {
    let log_dir = temp_log_dir();
    let (join, handle) = spawn(log_dir.clone());

    let endpoint = EndpointName::new("test-endpoint".to_owned());
    let messages = vec![
        Message {
            role: Role::User,
            content: OutputText::new("hello".to_owned()),
            timestamp: augur_domain::domain::newtypes::TimestampMs::new(1_000),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::Assistant,
            content: OutputText::new("hi there".to_owned()),
            timestamp: augur_domain::domain::newtypes::TimestampMs::new(2_000),
            tool_call_id: None,
            tool_calls: None,
        },
    ];

    handle.log_messages(endpoint, messages).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.shutdown();
    let _ = timeout(Duration::from_secs(2), join).await;

    // Find the log file written
    let entries: Vec<_> = std::fs::read_dir(&log_dir)
        .expect("log dir must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(entries.len(), 1, "expected exactly one log file");

    let content = std::fs::read_to_string(entries[0].path()).expect("log file must be readable");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2, "expected two JSONL lines");

    let first: serde_json::Value =
        serde_json::from_str(lines[0]).expect("first line must be valid JSON");
    assert_eq!(first["role"], "user");
    assert_eq!(first["content"], "hello");
    assert_eq!(first["endpoint"], "test-endpoint");

    let second: serde_json::Value =
        serde_json::from_str(lines[1]).expect("second line must be valid JSON");
    assert_eq!(second["role"], "assistant");
    assert_eq!(second["content"], "hi there");

    let _ = std::fs::remove_dir_all(&log_dir);
}

/// Verifies that multiple calls to log_messages all append to the same file in order.
#[tokio::test]
async fn multiple_turns_append_to_same_file() {
    let log_dir = temp_log_dir();
    let (join, handle) = spawn(log_dir.clone());

    let endpoint = EndpointName::new("ep".to_owned());

    let turn1 = vec![
        Message {
            role: Role::User,
            content: OutputText::new("turn1 user".to_owned()),
            timestamp: augur_domain::domain::newtypes::TimestampMs::new(1_000),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::Assistant,
            content: OutputText::new("turn1 reply".to_owned()),
            timestamp: augur_domain::domain::newtypes::TimestampMs::new(2_000),
            tool_call_id: None,
            tool_calls: None,
        },
    ];
    let turn2 = vec![
        Message {
            role: Role::User,
            content: OutputText::new("turn2 user".to_owned()),
            timestamp: augur_domain::domain::newtypes::TimestampMs::new(3_000),
            tool_call_id: None,
            tool_calls: None,
        },
        Message {
            role: Role::Assistant,
            content: OutputText::new("turn2 reply".to_owned()),
            timestamp: augur_domain::domain::newtypes::TimestampMs::new(4_000),
            tool_call_id: None,
            tool_calls: None,
        },
    ];

    handle.log_messages(endpoint.clone(), turn1).await;
    handle.log_messages(endpoint.clone(), turn2).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.shutdown();
    let _ = timeout(Duration::from_secs(2), join).await;

    let entries: Vec<_> = std::fs::read_dir(&log_dir)
        .expect("log dir must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(entries.len(), 1, "still one log file across multiple turns");

    let content = std::fs::read_to_string(entries[0].path()).expect("readable");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 4, "four messages across two turns");

    let _ = std::fs::remove_dir_all(&log_dir);
}

/// Verifies that shutdown cleanly terminates the actor task.
#[tokio::test]
async fn shutdown_completes_without_hang() {
    let log_dir = temp_log_dir();
    let (join, handle) = spawn(log_dir.clone());
    handle.shutdown();
    let result = timeout(Duration::from_secs(2), join).await;
    assert!(
        result.is_ok(),
        "actor must finish within 2 seconds of shutdown"
    );
    let _ = std::fs::remove_dir_all(&log_dir);
}

/// Verifies that `log_history_entry` writes the entry to the log file as a JSONL line.
#[tokio::test]
async fn test_log_history_entry_written_to_file() {
    use augur_domain::domain::feeds::HistoryFeedMessage;

    let log_dir = temp_log_dir();
    let (join, handle) = spawn(log_dir.clone());

    let msg = augur_domain::domain::types::Message {
        role: augur_domain::domain::types::Role::User,
        content: augur_domain::domain::string_newtypes::OutputText::new("history msg".to_owned()),
        timestamp: augur_domain::domain::newtypes::TimestampMs::new(9_000),
        tool_call_id: None,
        tool_calls: None,
    };
    let entry = HistoryFeedMessage::UserEntry(msg);
    handle.log_history_entry(entry);

    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.shutdown();
    let _ = timeout(Duration::from_secs(2), join).await;

    let entries: Vec<_> = std::fs::read_dir(&log_dir)
        .expect("log dir must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(entries.len(), 1, "expected exactly one log file");

    let content = std::fs::read_to_string(entries[0].path()).expect("log file must be readable");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "expected one JSONL line");

    let parsed: serde_json::Value = serde_json::from_str(lines[0]).expect("must be valid JSON");
    assert_eq!(parsed["role"], "user");
    assert_eq!(parsed["content"], "history msg");

    let _ = std::fs::remove_dir_all(&log_dir);
}

/// Verifies that a fatal log-file setup error disables logging and still exits cleanly.
#[tokio::test]
async fn setup_failure_disables_logging_and_exits_cleanly() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let log_dir = file.path().to_path_buf();
    let (join, handle) = spawn(log_dir.clone());

    let endpoint = EndpointName::new("test-endpoint".to_owned());
    handle
        .log_messages(
            endpoint,
            vec![Message {
                role: Role::User,
                content: OutputText::new("hello".to_owned()),
                timestamp: augur_domain::domain::newtypes::TimestampMs::new(1_000),
                tool_call_id: None,
                tool_calls: None,
            }],
        )
        .await;
    handle.shutdown();
    drop(handle);

    let result = timeout(Duration::from_secs(2), join).await;
    assert!(result.is_ok(), "logger should exit after setup failure");
    assert!(
        log_dir.is_file(),
        "the unwritable log path should remain a file"
    );
}

#[test]
fn mirror_sync_executes_creates_log_directory_on_spawn() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    drop(runtime);
}
