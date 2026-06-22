use augur_domain::domain::{
    Count, EndpointName, IsPredicate, LlmTokenCounts, LlmUsage, Message, MessageType,
    NumericNewtype, OutputText, PromptText, SessionId, StrategyNodeName, StringNewtype,
    Temperature, TimestampMs, TokenCount, ToolName,
};
use augur_domain::persistence::types::{
    summarize, MessageRecord, NodeMeta, SessionMeta, SessionMetaFlags, SessionRecord, SessionState,
    StrategyNode, StrategyNodeKind, StrategyTree,
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
        },
        state: SessionState::default(),
    }
}

#[test]
fn node_meta_new_sets_fields_and_timestamps() {
    let before = TimestampMs::now();
    let meta = NodeMeta::new("step1", "first step");
    let after = TimestampMs::now();
    assert_eq!(meta.name.as_str(), "step1");
    assert_eq!(meta.description.as_str(), "first step");
    assert!(meta.created_at >= before && meta.created_at <= after);
    assert!(meta.last_updated_at >= before && meta.last_updated_at <= after);
    assert!(meta.finished_at.is_none());
}

#[test]
fn strategy_tree_leaf_round_trips() {
    let mut nodes = std::collections::HashMap::new();
    nodes.insert(
        StrategyNodeName::new("leaf1"),
        StrategyNode {
            meta: NodeMeta::new("leaf1", "leaf node"),
            kind: StrategyNodeKind::Leaf(PromptText::new("final prompt text")),
        },
    );
    let tree = StrategyTree { nodes };
    let json = serde_json::to_string(&tree).expect("serialize");
    let back: StrategyTree = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.nodes.len(), 1);
    assert!(back.nodes.contains_key(&StrategyNodeName::new("leaf1")));
}

#[test]
fn strategy_tree_branch_round_trips() {
    let mut children = std::collections::HashMap::new();
    children.insert(
        StrategyNodeName::new("child"),
        StrategyNode {
            meta: NodeMeta::new("child", "child node"),
            kind: StrategyNodeKind::Leaf(PromptText::new("terminal")),
        },
    );
    let mut nodes = std::collections::HashMap::new();
    nodes.insert(
        StrategyNodeName::new("parent"),
        StrategyNode {
            meta: NodeMeta::new("parent", "parent node"),
            kind: StrategyNodeKind::Branch(children),
        },
    );
    let tree = StrategyTree { nodes };
    let json = serde_json::to_string(&tree).expect("serialize");
    let back: StrategyTree = serde_json::from_str(&json).expect("deserialize");
    match &back.nodes[&StrategyNodeName::new("parent")].kind {
        StrategyNodeKind::Branch(c) => assert!(c.contains_key(&StrategyNodeName::new("child"))),
        _ => panic!("expected Branch"),
    }
}

#[test]
fn strategy_tree_root_keys_use_strategy_node_name_newtype() {
    let tree: StrategyTree = serde_json::from_value(serde_json::json!({
        "nodes": {
            "branch-a": {
                "meta": {
                    "name": "branch-a",
                    "description": "first branch",
                    "created_at": 1,
                    "last_updated_at": 1,
                    "finished_at": null
                },
                "kind": { "Leaf": "prompt text" }
            }
        }
    }))
    .expect("strategy tree JSON must deserialize");

    let key = tree.nodes.keys().next().expect("root key must exist");
    let key_type = std::any::type_name_of_val(key);
    assert!(key_type.contains("StrategyNodeName"));
}

#[test]
fn strategy_tree_branch_keys_use_strategy_node_name_newtype() {
    let tree: StrategyTree = serde_json::from_value(serde_json::json!({
        "nodes": {
            "branch-a": {
                "meta": {
                    "name": "branch-a",
                    "description": "first branch",
                    "created_at": 1,
                    "last_updated_at": 1,
                    "finished_at": null
                },
                "kind": {
                    "Branch": {
                        "child-b": {
                            "meta": {
                                "name": "child-b",
                                "description": "second branch",
                                "created_at": 1,
                                "last_updated_at": 1,
                                "finished_at": null
                            },
                            "kind": { "Leaf": "prompt text" }
                        }
                    }
                }
            }
        }
    }))
    .expect("strategy tree JSON must deserialize");

    let branch = tree.nodes.values().next().expect("branch node must exist");
    let StrategyNodeKind::Branch(children) = &branch.kind else {
        panic!("expected branch node");
    };
    let child_key = children.keys().next().expect("child key must exist");
    let child_key_type = std::any::type_name_of_val(child_key);
    assert!(child_key_type.contains("StrategyNodeName"));
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
    assert!(record.state.current_strategy.is_none());
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
    assert_eq!(summary.preview.as_str(), "");
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
