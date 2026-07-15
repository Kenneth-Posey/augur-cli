use augur_domain::domain::{
    Count, EndpointName, IsPredicate, LlmTokenCounts, LlmUsage, Message, MessageType,
    NumericNewtype, OutputText, SessionId, StringNewtype, Temperature, TimestampMs, TokenCount,
    ToolName,
};
use augur_domain::persistence::types::{
    MessageRecord, SessionMeta, SessionMetaFlags, SessionRecord, SessionState, summarize,
};

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
            title: None,
        },
        state: SessionState::default(),
    }
}

#[test]
fn message_type_all_variants_round_trip() {
    let usage = LlmUsage {
        model: OutputText::new("claude-test"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(10),
            tokens_out: TokenCount::new(5),
            tokens_cached: TokenCount::new(0),
            cache_write_tokens: TokenCount::new(0),
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.7),
    };
    let variants: Vec<MessageType> = vec![
        MessageType::User,
        MessageType::Tool(ToolName::new("bash")),
        MessageType::Assistant,
        MessageType::LlmResponse(usage),
        MessageType::Error,
        MessageType::System,
    ];
    for variant in &variants {
        let json = serde_json::to_string(variant).expect("serialize");
        let back: MessageType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, *variant);
    }
}

#[test]
fn session_record_new_has_empty_state_and_uuid() {
    let record = make_record("test-endpoint");
    assert!(!record.meta.id.as_str().is_empty());
    assert_eq!(record.meta.endpoint_name.as_str(), "test-endpoint");
    assert!(record.state.messages.is_empty());
}

#[test]
fn session_record_new_generates_unique_ids() {
    let a = make_record("ep");
    let b = make_record("ep");
    assert_ne!(a.meta.id.as_str(), b.meta.id.as_str());
}

#[test]
fn session_record_round_trips() {
    let record = make_record("anthropic");
    let json = serde_json::to_string(&record).expect("serialize");
    let back: SessionRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.meta.id.as_str(), record.meta.id.as_str());
    assert_eq!(back.meta.endpoint_name.as_str(), "anthropic");
}

#[test]
fn summarize_empty_messages_returns_empty_preview() {
    let record = make_record("ep");
    let summary = summarize(&record);
    assert_eq!(summary.preview.as_str(), "<<no prompt>>");
    assert_eq!(summary.message_count, Count::new(0));
}

#[test]
fn summarize_returns_first_message_preview_and_count() {
    let mut record = make_record("ep");
    let msg = Message::user("short message");
    record.state.messages.push(MessageRecord {
        message_type: MessageType::User,
        message: msg,
    });
    let summary = summarize(&record);
    assert_eq!(summary.preview.as_str(), "short message");
    assert_eq!(summary.message_count, Count::new(1));
}

#[test]
fn summarize_copies_identity_fields() {
    let record = make_record("gpt-4");
    let summary = summarize(&record);
    assert_eq!(summary.identity.id.as_str(), record.meta.id.as_str());
    assert_eq!(summary.identity.endpoint_name.as_str(), "gpt-4");
    assert_eq!(summary.identity.created_at, record.meta.created_at);
}

#[test]
fn summarize_unicode_multibyte_message_does_not_panic() {
    let mut long_text = String::new();
    for _ in 0..10 {
        long_text.push('a');
        long_text.push('\u{2013}');
    }
    long_text.push_str(&"b".repeat(30));
    let mut record = make_record("ep");
    record.state.messages.push(MessageRecord {
        message_type: MessageType::User,
        message: Message::user(long_text.as_str()),
    });
    let summary = summarize(&record);
    assert!(!summary.preview.as_str().is_empty());
}
