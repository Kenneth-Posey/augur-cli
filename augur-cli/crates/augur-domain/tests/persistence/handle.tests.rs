use augur_domain::domain::{
    EndpointName, IsPredicate, LlmTokenCounts, LlmUsage, Message, MessageType, NumericNewtype,
    OutputText, PromptText, Role, SdkSessionId, SessionId, StringNewtype, Temperature, TimestampMs,
    TokenCount,
};
use augur_domain::persistence::handle::PersistenceHandle;
use augur_domain::persistence::store;
use augur_domain::persistence::types::{
    MessageRecord, SessionMeta, SessionMetaFlags, SessionRecord, SessionState,
};
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("tempdir creation failed")
}

fn make_record(endpoint: &str) -> SessionRecord {
    SessionRecord {
        meta: SessionMeta {
            id: SessionId::new(uuid::Uuid::new_v4().to_string()),
            created_at: TimestampMs::now(),
            last_updated_at: TimestampMs::now(),
            endpoint_name: EndpointName::new(endpoint),
            flags: SessionMetaFlags {
                sdk_session_id: None,
                ask_session: IsPredicate::from(false),
            },
        },
        state: SessionState::default(),
    }
}

#[test]
fn new_handle_has_non_empty_session_id() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    assert!(!handle.session_id().as_str().is_empty());
}

#[test]
fn two_new_handles_have_distinct_ids() {
    let dir = temp_dir();
    let a = PersistenceHandle::new(dir.path().to_owned());
    let b = PersistenceHandle::new(dir.path().to_owned());
    assert_ne!(a.session_id().as_str(), b.session_id().as_str());
}

#[test]
fn restore_from_replaces_session_id() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let original_id = handle.session_id();
    let record = make_record("ep");
    handle.restore_from(&record);
    let restored_id = handle.session_id();
    assert_ne!(original_id.as_str(), restored_id.as_str());
    assert_eq!(restored_id.as_str(), record.meta.id.as_str());
}

#[tokio::test]
async fn save_turn_writes_file_to_disk() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let id = handle.session_id();
    handle.save_turn(EndpointName::new("ep"), vec![]).await;
    let path = dir.path().join(format!("{}.json", id.as_str()));
    assert!(path.exists());
}

#[tokio::test]
async fn save_turn_after_restore_uses_restored_id() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let record = make_record("ep");
    let restored_id = record.meta.id.as_str().to_owned();
    handle.restore_from(&record);
    handle.save_turn(EndpointName::new("ep"), vec![]).await;
    let path = dir.path().join(format!("{restored_id}.json"));
    assert!(path.exists());
}

#[test]
fn reset_to_new_session_generates_new_id() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let original_id = handle.session_id();
    handle.reset_to_new_session();
    let new_id = handle.session_id();
    assert_ne!(original_id.as_str(), new_id.as_str());
}

#[test]
fn reset_to_new_session_clears_sdk_session_id() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    handle.set_sdk_session_id(SdkSessionId::new("existing-sdk-session"));
    assert!(handle.sdk_session_id().is_some());
    handle.reset_to_new_session();
    assert!(handle.sdk_session_id().is_none());
}

#[tokio::test]
async fn mark_as_ask_session_flag_persists_in_saved_file() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    handle.mark_as_ask_session();
    handle.save_turn(EndpointName::new("ep"), vec![]).await;
    let id = handle.session_id();
    let loaded = store::load_session(dir.path(), &id).expect("load_session failed");
    assert!(loaded.meta.flags.ask_session.0);
}

#[tokio::test]
async fn mark_as_ask_session_excluded_from_list_sessions() {
    let dir = temp_dir();

    let regular = PersistenceHandle::new(dir.path().to_owned());
    regular
        .save_turn(EndpointName::new("ep-regular"), vec![])
        .await;

    let ask = PersistenceHandle::new(dir.path().to_owned());
    ask.mark_as_ask_session();
    ask.save_turn(EndpointName::new("ep-ask"), vec![]).await;

    let list = store::list_sessions(dir.path()).expect("list_sessions failed");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].identity.endpoint_name.as_str(), "ep-regular");
}

#[tokio::test]
async fn save_turn_preserves_message_type() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let id = handle.session_id();
    let usage = LlmUsage {
        model: OutputText::new("test-model"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(10),
            tokens_out: TokenCount::new(5),
            tokens_cached: TokenCount::new(2),
            cache_write_tokens: TokenCount::new(0),
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.7),
    };
    let records = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user(PromptText::new("hello")),
        },
        MessageRecord {
            message_type: MessageType::LlmResponse(usage.clone()),
            message: Message::assistant(OutputText::new("world")),
        },
    ];
    handle.save_turn(EndpointName::new("ep"), records).await;

    let loaded = store::load_session(dir.path(), &id).expect("load_session failed");
    let msgs = &loaded.state.messages;
    assert_eq!(msgs.len(), 2);
    match &msgs[1].message_type {
        MessageType::LlmResponse(u) => {
            assert_eq!(u.tokens_in, usage.tokens_in);
            assert_eq!(u.tokens_out, usage.tokens_out);
            assert_eq!(u.tokens_cached, usage.tokens_cached);
        }
        other => panic!("expected LlmResponse, got {other:?}"),
    }
}

#[tokio::test]
async fn queued_commands_appear_in_saved_session() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let id = handle.session_id();
    let ts = TimestampMs::now();
    handle.queue_user_command(MessageRecord {
        message_type: MessageType::User,
        message: Message {
            role: Role::User,
            content: OutputText::new("/switch-endpoint foo"),
            timestamp: ts,
            tool_call_id: None,
            tool_calls: None,
        },
    });
    handle.save_turn(EndpointName::new("ep"), vec![]).await;

    let loaded = store::load_session(dir.path(), &id).expect("load_session failed");
    assert_eq!(loaded.state.messages.len(), 1);
    assert_eq!(
        loaded.state.messages[0].message.content.as_str(),
        "/switch-endpoint foo"
    );
    assert!(matches!(
        loaded.state.messages[0].message_type,
        MessageType::User
    ));
}

#[tokio::test]
async fn queued_commands_cleared_after_save_turn() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let id = handle.session_id();
    let ts = TimestampMs::now();
    handle.queue_user_command(MessageRecord {
        message_type: MessageType::User,
        message: Message {
            role: Role::User,
            content: OutputText::new("/run-pipeline"),
            timestamp: ts,
            tool_call_id: None,
            tool_calls: None,
        },
    });

    handle.save_turn(EndpointName::new("ep"), vec![]).await;
    let first = store::load_session(dir.path(), &id).expect("load_session failed");
    assert_eq!(first.state.messages.len(), 1);

    handle.save_turn(EndpointName::new("ep"), vec![]).await;
    let second = store::load_session(dir.path(), &id).expect("load_session failed");
    assert_eq!(second.state.messages.len(), 0);
}

#[tokio::test]
async fn queued_commands_sorted_by_timestamp() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let id = handle.session_id();
    let early_ts = TimestampMs::new(1_000);
    let late_ts = TimestampMs::new(2_000);
    handle.queue_user_command(MessageRecord {
        message_type: MessageType::User,
        message: Message {
            role: Role::User,
            content: OutputText::new("/switch-endpoint early"),
            timestamp: early_ts,
            tool_call_id: None,
            tool_calls: None,
        },
    });
    let agent_msg = MessageRecord {
        message_type: MessageType::User,
        message: Message {
            role: Role::User,
            content: OutputText::new("later prompt"),
            timestamp: late_ts,
            tool_call_id: None,
            tool_calls: None,
        },
    };
    handle
        .save_turn(EndpointName::new("ep"), vec![agent_msg])
        .await;

    let loaded = store::load_session(dir.path(), &id).expect("load_session failed");
    assert_eq!(loaded.state.messages.len(), 2);
    assert_eq!(
        loaded.state.messages[0].message.content.as_str(),
        "/switch-endpoint early"
    );
    assert_eq!(
        loaded.state.messages[1].message.content.as_str(),
        "later prompt"
    );
}

#[tokio::test]
async fn openrouter_context_history_persists_in_saved_file() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let id = handle.session_id();
    handle.set_openrouter_context_history(vec![Message::assistant(OutputText::new("ctx entry"))]);
    handle
        .save_turn(EndpointName::new("openrouter"), vec![])
        .await;

    let loaded = store::load_session(dir.path(), &id).expect("load_session failed");
    let ctx = loaded
        .state
        .openrouter_context_history
        .expect("openrouter context history should be present");
    assert_eq!(ctx.len(), 1);
    assert_eq!(ctx[0].content.as_str(), "ctx entry");
}

#[test]
fn restore_from_hydrates_openrouter_context_history() {
    let dir = temp_dir();
    let handle = PersistenceHandle::new(dir.path().to_owned());
    let mut record = make_record("openrouter");
    record.state.openrouter_context_history =
        Some(vec![Message::assistant(OutputText::new("restored ctx"))]);

    handle.restore_from(&record);
    let ctx = handle
        .openrouter_context_history()
        .expect("context history should be restored");
    assert_eq!(ctx.len(), 1);
    assert_eq!(ctx[0].content.as_str(), "restored ctx");
}
