use augur_core::actors::agent::persistence_ops::{
    MessageContext, build_message_records, make_error_annotation, merge_with_error_annotations,
};
use augur_core::persistence::{MessageRecord, MessageType};
use augur_domain::domain::newtypes::{Count, NumericNewtype};
use augur_domain::domain::string_newtypes::{OutputText, PromptText, StringNewtype};
use augur_domain::domain::types::{LlmTokenCounts, LlmUsage, Message, Role};
use augur_domain::domain::{Temperature, TokenCount};

#[test]
fn build_message_records_user_message() {
    let messages = vec![Message::user(PromptText::new("hello"))];
    let records = build_message_records(&messages, None);
    assert_eq!(records.len(), 1);
    assert!(matches!(records[0].message_type, MessageType::User));
}

#[test]
fn build_message_records_assistant_message() {
    let messages = vec![
        Message::user(PromptText::new("hello")),
        Message::assistant(OutputText::new("response")),
    ];
    let records = build_message_records(&messages, None);
    assert_eq!(records.len(), 2);
    assert!(matches!(records[1].message_type, MessageType::Assistant));
}

#[test]
fn build_message_records_last_assistant_with_usage() {
    let messages = vec![
        Message::user(PromptText::new("hello")),
        Message::assistant(OutputText::new("response")),
    ];
    let usage = LlmUsage {
        model: OutputText::new("test-model"),
        token_counts: LlmTokenCounts {
            tokens_in: TokenCount::new(10),
            tokens_out: TokenCount::new(20),
            tokens_cached: TokenCount::new(0),
            cache_write_tokens: TokenCount::new(0),
            cost_usd: 0.0.into(),
        },
        temperature: Temperature::new(0.7),
    };
    let records = build_message_records(&messages, Some(usage.clone()));
    assert_eq!(records.len(), 2);
    match &records[1].message_type {
        MessageType::LlmResponse(u) => {
            assert_eq!(u.tokens_in, usage.tokens_in);
        }
        _ => panic!("expected LlmResponse"),
    }
}

#[test]
fn merge_with_error_annotations_insertion() {
    let base = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user(PromptText::new("msg1")),
        },
        MessageRecord {
            message_type: MessageType::Assistant,
            message: Message::assistant(OutputText::new("msg2")),
        },
    ];
    let annotations = vec![(
        Count::new(2),
        MessageRecord {
            message_type: MessageType::Error,
            message: Message {
                role: Role::System,
                content: OutputText::new("error"),
                timestamp: augur_domain::domain::TimestampMs::now(),
                tool_call_id: None,
                tool_calls: None,
            },
        },
    )];
    let result = merge_with_error_annotations(base, &annotations);
    assert_eq!(result.len(), 3);
    assert!(matches!(result[2].message_type, MessageType::Error));
}

#[test]
fn merge_with_error_annotations_empty_no_change() {
    let base = vec![MessageRecord {
        message_type: MessageType::User,
        message: Message::user(PromptText::new("msg1")),
    }];
    let annotations: Vec<(Count, MessageRecord)> = vec![];
    let result = merge_with_error_annotations(base.clone(), &annotations);
    assert_eq!(result.len(), 1);
    assert!(matches!(result[0].message_type, MessageType::User));
}

#[test]
fn make_error_annotation_creates_system_error() {
    let error_text = OutputText::new("An error occurred");
    let record = make_error_annotation(error_text.clone());
    assert!(matches!(record.message_type, MessageType::Error));
    assert_eq!(record.message.role, Role::System);
    assert_eq!(record.message.content, error_text);
}

#[test]
fn message_context_builder() {
    let usage: Option<LlmUsage> = None;
    let ctx = MessageContext::builder()
        .idx(0)
        .maybe_last_assistant_idx(Some(0))
        .last_usage(&usage)
        .build();
    assert_eq!(ctx.idx, 0);
    assert_eq!(ctx.last_assistant_idx, Some(0));
}

#[test]
fn merge_with_error_annotations_beyond_range() {
    let base = vec![
        MessageRecord {
            message_type: MessageType::User,
            message: Message::user(PromptText::new("msg1")),
        },
        MessageRecord {
            message_type: MessageType::Assistant,
            message: Message::assistant(OutputText::new("msg2")),
        },
    ];

    let annotations = vec![(
        Count::new(5),
        MessageRecord {
            message_type: MessageType::Error,
            message: Message {
                role: Role::System,
                content: OutputText::new("error beyond range"),
                timestamp: augur_domain::domain::TimestampMs::now(),
                tool_call_id: None,
                tool_calls: None,
            },
        },
    )];

    let result = merge_with_error_annotations(base, &annotations);
    assert_eq!(result.len(), 3);
    assert!(matches!(result[0].message_type, MessageType::User));
    assert!(matches!(result[1].message_type, MessageType::Assistant));
    assert!(matches!(result[2].message_type, MessageType::Error));
    assert_eq!(
        result[2].message.content,
        OutputText::new("error beyond range")
    );
}
