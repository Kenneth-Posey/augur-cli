#![allow(clippy::duplicate_mod)]
use augur_domain::domain::newtypes::{NumericNewtype, Temperature, TokenCount, UsdCost};
use augur_domain::domain::string_newtypes::{
    ConversationId, FilePath, OutputText, PromptText, StringNewtype, ToolCallId, ToolName,
};
use augur_domain::domain::types::{
    AgentFeedOutput, CommandOutcome, FeedEntry, FeedId, FileCompletion, LlmTokenCounts, LlmUsage,
    Message, ProjectTokenTotals, Role, RouteResult, StreamChunk,
};

#[path = "../support/rustdoc.tests.rs"]
mod rustdoc_support;

/// Verifies Message::user produces a message with Role::User.
#[test]
fn message_user_role() {
    let msg = Message::user(PromptText::new("hi"));
    assert_eq!(msg.role, Role::User);
}

/// Verifies Message::assistant produces a message with Role::Assistant.
#[test]
fn message_assistant_role() {
    let msg = Message::assistant(OutputText::new("response"));
    assert_eq!(msg.role, Role::Assistant);
}

/// Verifies Message::system produces a message with Role::System.
#[test]
fn message_system_role() {
    let msg = Message::system(OutputText::new("you are helpful"));
    assert_eq!(msg.role, Role::System);
}

/// Verifies Message::tool_result produces a message with Role::Tool.
#[test]
fn message_tool_result_role() {
    let name = ToolName::new("my_tool");
    let msg = Message::tool_result(
        ToolCallId::new("call_test"),
        &name,
        OutputText::new("result"),
    );
    assert_eq!(msg.role, Role::Tool);
}

/// Verifies tool result message content is prefixed with "[name]: ".
#[test]
fn message_tool_result_prefixes_name() {
    let name = ToolName::new("my_tool");
    let msg = Message::tool_result(
        ToolCallId::new("call_test"),
        &name,
        OutputText::new("output here"),
    );
    assert!(
        msg.content.as_str().starts_with("[my_tool]: "),
        "Expected prefix '[my_tool]: ', got: {}",
        msg.content.as_str()
    );
}

/// Verifies all Message constructors stamp a positive timestamp.
#[test]
fn message_timestamps_are_set() {
    assert!(Message::user(PromptText::new("x")).timestamp.inner() > 0);
}

/// Verifies two ConversationId::generate() calls produce different values.
#[test]
fn conversation_id_two_calls_differ() {
    assert_ne!(ConversationId::generate(), ConversationId::generate());
}

/// Verifies all StreamChunk variants can be constructed without panic.
#[test]
fn stream_chunk_variants_construct() {
    let _token = StreamChunk::Token(OutputText::new("tok"));
    let _call = StreamChunk::ToolCall {
        id: ToolCallId::new(""),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({"command": "ls"}),
    };
    let _done = StreamChunk::Done;
    let _err = StreamChunk::Error(OutputText::new("oops"));
}

/// Verifies FileCompletion can be constructed and fields are accessible.
#[test]
fn file_completion_construction() {
    let fc = FileCompletion {
        path: FilePath::new("src/main.rs"),
        display_name: "main.rs".to_owned().into(),
    };
    assert_eq!(fc.path.as_str(), "src/main.rs");
    assert_eq!(fc.display_name, "main.rs");
}

/// Verifies FileCompletion derives Clone correctly.
#[test]
fn file_completion_clone() {
    let fc = FileCompletion {
        path: FilePath::new("src/lib.rs"),
        display_name: "lib.rs".to_owned().into(),
    };
    let cloned = fc.clone();
    assert_eq!(cloned.path, fc.path);
    assert_eq!(cloned.display_name, fc.display_name);
}

/// Verifies FileCompletion derives PartialEq correctly.
#[test]
fn file_completion_equality() {
    let a = FileCompletion {
        path: FilePath::new("a.rs"),
        display_name: "a.rs".to_owned().into(),
    };
    let b = FileCompletion {
        path: FilePath::new("a.rs"),
        display_name: "a.rs".to_owned().into(),
    };
    let c = FileCompletion {
        path: FilePath::new("b.rs"),
        display_name: "b.rs".to_owned().into(),
    };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

/// Verifies FileCompletion Debug formatting includes path and display_name.
#[test]
fn file_completion_debug() {
    let fc = FileCompletion {
        path: FilePath::new("src/foo.rs"),
        display_name: "foo.rs".to_owned().into(),
    };
    let s = format!("{:?}", fc);
    assert!(s.contains("src/foo.rs"));
    assert!(s.contains("foo.rs"));
}

/// Verifies that CommandOutcome::RunBackgroundAgent can be constructed and
/// destructured, confirming the variant holds expected semantic fields.
#[test]
fn run_background_agent_variant_constructs() {
    let v = CommandOutcome::RunBackgroundAgent {
        agent: "x".into(),
        prompt: "y".into(),
    };
    match v {
        CommandOutcome::RunBackgroundAgent { agent, prompt } => {
            assert_eq!(
                agent.as_str(),
                "x",
                "agent field must round-trip through construction"
            );
            assert_eq!(
                prompt.as_str(),
                "y",
                "prompt field must round-trip through construction"
            );
        }
        _ => panic!("RunBackgroundAgent variant did not match after construction"),
    }
}

/// FeedId::Agent variant is identifiable via pattern match.
#[test]
fn feed_id_agent_is_agent_feed() {
    assert!(matches!(FeedId::Agent("tc1".into()), FeedId::Agent(_)));
}

/// FeedId::MainConversation is not the Agent variant.
#[test]
fn feed_id_main_is_not_agent_feed() {
    assert!(!matches!(FeedId::MainConversation, FeedId::Agent(_)));
}

/// FeedEntry carries feed_id and output fields.
#[test]
fn feed_entry_carries_feed_id() {
    let entry = FeedEntry {
        feed_id: FeedId::Agent("tc1".into()),
        output: AgentFeedOutput::StatusLine(OutputText::new("hello".to_owned())),
    };
    assert!(matches!(entry.feed_id, FeedId::Agent(_)));
    assert!(matches!(entry.output, AgentFeedOutput::StatusLine(_)));
}

/// RouteResult can be constructed with both fields None.
#[test]
fn route_result_both_none() {
    let r = RouteResult {
        main_out: None,
        feed_out: None,
    };
    assert!(r.main_out.is_none());
    assert!(r.feed_out.is_none());
}

/// Verifies Phase 1 domain types expose newtype-based public APIs in rustdoc.
#[test]
fn domain_types_public_api_uses_phase_one_newtypes() {
    let command_outcome_html =
        rustdoc_support::rustdoc_html("augur_domain/domain/types/enum.CommandOutcome.html");
    assert!(
        command_outcome_html.contains("struct.FilePath.html"),
        "expected CommandOutcome rustdoc to reference FilePath",
    );
    assert!(
        command_outcome_html.contains("struct.AgentName.html"),
        "expected CommandOutcome rustdoc to reference AgentName",
    );
    assert!(
        command_outcome_html.contains("struct.PromptText.html"),
        "expected CommandOutcome rustdoc to reference PromptText",
    );
}

/// Verifies LlmUsage deserializes successfully when cache_write_tokens and cost_usd are absent.
#[test]
fn test_llm_usage_serde_defaults_cost_usd_is_zero() {
    let json = r#"{"model":"m","tokens_in":1,"tokens_out":1,"tokens_cached":0,"temperature":0.0}"#;
    let result: Result<LlmUsage, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "LlmUsage must deserialize without cache_write_tokens and cost_usd"
    );
    let u = result.unwrap();
    assert_eq!(u.cache_write_tokens, TokenCount::ZERO);
    assert_eq!(u.cost_usd, UsdCost::ZERO);
}

/// Verifies ProjectTokenTotals deserializes successfully when new fields are absent.
#[test]
fn test_project_token_totals_serde_defaults_missing_fields() {
    let json = r#"{"tokens_in":5,"tokens_out":3,"tokens_cached":1}"#;
    let result: Result<ProjectTokenTotals, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "ProjectTokenTotals must deserialize from earlier-schema JSON"
    );
    let t = result.unwrap();
    assert_eq!(t.cache_write_tokens, TokenCount::ZERO);
    assert_eq!(t.cost_usd, UsdCost::ZERO);
}

/// Verifies ProjectTokenTotals deserializes from an empty object.
#[test]
fn test_project_token_totals_serde_defaults_from_empty_object() {
    let json = "{}";
    let result: Result<ProjectTokenTotals, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "ProjectTokenTotals must deserialize from empty JSON object"
    );
    let t = result.unwrap();
    assert_eq!(t.tokens_in, TokenCount::ZERO);
    assert_eq!(t.tokens_out, TokenCount::ZERO);
    assert_eq!(t.tokens_cached, TokenCount::ZERO);
    assert_eq!(t.cache_write_tokens, TokenCount::ZERO);
    assert_eq!(t.cost_usd, UsdCost::ZERO);
}

/// Verifies ProjectTokenTotals::default() has all zero values.
#[test]
fn test_project_token_totals_default_all_zero() {
    let t = ProjectTokenTotals::default();
    assert_eq!(t.tokens_in, TokenCount::ZERO);
    assert_eq!(t.tokens_out, TokenCount::ZERO);
    assert_eq!(t.tokens_cached, TokenCount::ZERO);
    assert_eq!(t.cache_write_tokens, TokenCount::ZERO);
    assert_eq!(t.cost_usd, UsdCost::ZERO);
}

use proptest::prelude::*;

proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]

    /// Property: LlmUsage serde round-trips without data loss.
    #[test]
    fn prop_llm_usage_serde_round_trip(
        in_tok  in 0u64..100_000,
        out_tok in 0u64..100_000,
        cached  in 0u64..100_000,
        writes  in 0u64..100_000,
        cost    in 0.0f64..1_000.0,
    ) {
        let original = LlmUsage {
            model: OutputText::new("test-model"),
            token_counts: LlmTokenCounts {
                tokens_in: TokenCount::new(in_tok),
                tokens_out: TokenCount::new(out_tok),
                tokens_cached: TokenCount::new(cached),
                cache_write_tokens: TokenCount::new(writes),
                cost_usd: cost.into(),
            },
            temperature: Temperature::new(0.7),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: LlmUsage = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored.tokens_in, original.tokens_in);
        prop_assert_eq!(restored.tokens_out, original.tokens_out);
        prop_assert_eq!(restored.tokens_cached, original.tokens_cached);
        prop_assert_eq!(restored.cache_write_tokens, original.cache_write_tokens);
        prop_assert!((restored.cost_usd - original.cost_usd).abs() < 1e-9);
    }

    /// Property: ProjectTokenTotals serde round-trips without data loss.
    #[test]
    fn prop_project_token_totals_serde_round_trip(
        in_tok  in 0u64..100_000,
        out_tok in 0u64..100_000,
        cached  in 0u64..100_000,
        writes  in 0u64..100_000,
        cost    in 0.0f64..1_000.0,
    ) {
        let original = ProjectTokenTotals {
            tokens_in: TokenCount::new(in_tok),
            tokens_out: TokenCount::new(out_tok),
            tokens_cached: TokenCount::new(cached),
            cache_write_tokens: TokenCount::new(writes),
            cost_usd: cost.into(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: ProjectTokenTotals = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(restored.tokens_in, original.tokens_in);
        prop_assert_eq!(restored.tokens_out, original.tokens_out);
        prop_assert_eq!(restored.tokens_cached, original.tokens_cached);
        prop_assert_eq!(restored.cache_write_tokens, original.cache_write_tokens);
        prop_assert!((restored.cost_usd - original.cost_usd).abs() < 1e-9);
    }
}
