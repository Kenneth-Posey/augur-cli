use augur_core::persistence::SessionRecord;
use augur_core::persistence::handle::PersistenceHandle;
use augur_core::persistence::store::{
    delete_session, list_sessions, load_session, resolve_sessions_dir, save_session,
};
use augur_domain::domain::{
    EndpointName, FilePath, IsPredicate, NumericNewtype, SessionId, StringNewtype, TimestampMs,
};
use std::path::PathBuf;
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("tempdir creation failed")
}

fn make_record(endpoint: &str) -> SessionRecord {
    SessionRecord {
        meta: augur_core::persistence::SessionMeta {
            id: SessionId::new(uuid::Uuid::new_v4().to_string()),
            created_at: TimestampMs::now(),
            last_updated_at: TimestampMs::now(),
            endpoint_name: EndpointName::new(endpoint),
            flags: augur_core::persistence::SessionMetaFlags {
                sdk_session_id: None,
                ask_session: IsPredicate::from(false),
            },
        },
        state: augur_core::persistence::SessionState::default(),
    }
}

#[test]
fn save_and_load_round_trips() {
    let dir = temp_dir();
    let record = make_record("test-ep");
    let id = record.meta.id.clone();
    save_session(&record, dir.path()).expect("save");
    let loaded = load_session(dir.path(), &id).expect("load");
    assert_eq!(loaded.meta.id.as_str(), record.meta.id.as_str());
    assert_eq!(loaded.meta.endpoint_name.as_str(), "test-ep");
}

#[test]
fn resolve_sessions_dir_none_returns_xdg_default() {
    let path = resolve_sessions_dir(None);
    let path_str = path.to_string_lossy();
    assert!(path_str.ends_with(".augur-cli/sessions"));
}

#[test]
fn resolve_sessions_dir_absolute_path_passthrough() {
    let path = resolve_sessions_dir(Some(&FilePath::new("/custom/sessions")));
    assert_eq!(path, PathBuf::from("/custom/sessions"));
}

#[test]
fn resolve_sessions_dir_tilde_prefix_expands_to_home() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    let path = resolve_sessions_dir(Some(&FilePath::new("~/my-sessions")));
    let expected = PathBuf::from(&home).join("my-sessions");
    assert_eq!(path, expected);
}

#[test]
fn resolve_sessions_dir_bare_tilde_resolves_to_home() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    let path = resolve_sessions_dir(Some(&FilePath::new("~")));
    assert_eq!(path, PathBuf::from(&home));
}

#[test]
fn list_sessions_returns_all_saved() {
    let dir = temp_dir();
    save_session(&make_record("ep-a"), dir.path()).expect("save a");
    save_session(&make_record("ep-b"), dir.path()).expect("save b");
    let list = list_sessions(dir.path()).expect("list");
    assert_eq!(list.len(), 2);
}

#[test]
fn list_sessions_missing_dir_returns_empty() {
    let dir = temp_dir();
    let missing = dir.path().join("nonexistent");
    let list = list_sessions(&missing).expect("list missing dir");
    assert!(list.is_empty());
}

#[test]
fn list_sessions_caps_at_twenty() {
    let dir = temp_dir();
    for _ in 0..25 {
        save_session(&make_record("ep"), dir.path()).expect("save");
    }
    let list = list_sessions(dir.path()).expect("list");
    assert!(list.len() <= 20);
}

#[test]
fn newest_first_ordering() {
    let dir = temp_dir();
    let mut record_a = make_record("ep-a");
    record_a.meta.last_updated_at = TimestampMs::new(1_000);
    record_a.meta.created_at = TimestampMs::new(3_000);

    let mut record_b = make_record("ep-b");
    record_b.meta.last_updated_at = TimestampMs::new(4_000);
    record_b.meta.created_at = TimestampMs::new(500);
    let id_b = record_b.meta.id.clone();

    save_session(&record_a, dir.path()).expect("save a");
    save_session(&record_b, dir.path()).expect("save b");

    let list = list_sessions(dir.path()).expect("list");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].identity.id.as_str(), id_b.as_str());
}

#[test]
fn list_sessions_excludes_ask_sessions() {
    let dir = temp_dir();
    let regular = make_record("ep-regular");
    save_session(&regular, dir.path()).expect("save regular");

    let mut ask = make_record("ep-ask");
    ask.meta.flags.ask_session = true.into();
    save_session(&ask, dir.path()).expect("save ask");

    let list = list_sessions(dir.path()).expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].identity.endpoint_name.as_str(), "ep-regular");
}

#[test]
fn delete_session_removes_saved_file() {
    let dir = temp_dir();
    let record = make_record("ep-delete");
    let id = record.meta.id.clone();
    save_session(&record, dir.path()).expect("save");
    delete_session(dir.path(), &id).expect("delete");
    assert!(load_session(dir.path(), &id).is_err());
}

#[test]
fn delete_session_missing_file_is_ok() {
    let dir = temp_dir();
    let missing = SessionId::new("does-not-exist");
    delete_session(dir.path(), &missing).expect("delete missing should succeed");
}

#[tokio::test]
async fn save_creates_missing_dir() {
    let dir = temp_dir();
    let sessions_dir = dir.path().join("sessions");
    assert!(!sessions_dir.exists());

    let persistence = PersistenceHandle::new(sessions_dir.clone());
    persistence.save_turn(EndpointName::new("ep"), vec![]).await;

    assert!(sessions_dir.exists());
    let entry_count = std::fs::read_dir(&sessions_dir)
        .expect("read_dir")
        .filter_map(|e| e.ok())
        .count();
    assert_eq!(entry_count, 1);
}
