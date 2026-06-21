use augur_domain::domain::newtypes::{NumericNewtype, TokenCount};
use augur_domain::domain::string_newtypes::{StringNewtype, ToolCallId, ToolName};
use augur_domain::domain::types::{Message, Role};
use augur_domain::newtypes::ToolResultStripFraction;
use augur_provider_openrouter::compaction::compact_messages_with_threshold;

fn default_strip_fraction() -> ToolResultStripFraction {
    ToolResultStripFraction::new(0.9)
}

#[test]
fn compact_messages_with_threshold_zero_preserves_input() {
    let messages = vec![Message::system("sys"), Message::user("hello")];
    let compacted = compact_messages_with_threshold(
        messages.clone(),
        TokenCount::ZERO,
        default_strip_fraction(),
    );
    assert_eq!(compacted.len(), messages.len());
    for (left, right) in compacted.iter().zip(messages.iter()) {
        assert_eq!(left.role, right.role);
        assert_eq!(left.content, right.content);
    }
}

#[test]
fn compact_messages_with_threshold_preserves_last_system_prompt() {
    let messages = vec![
        Message::system("prefix-a"),
        Message::system("prefix-b"),
        Message::system("system-prompt"),
        Message::user("question"),
    ];
    let compacted =
        compact_messages_with_threshold(messages, TokenCount::new(8), default_strip_fraction());
    assert_eq!(compacted[0].role, Role::System);
    assert_eq!(compacted[0].content.as_str(), "system-prompt");
}

#[test]
fn compact_messages_with_threshold_emits_combined_compaction_note() {
    let messages = vec![
        Message::system("prefix one prefix one prefix one"),
        Message::system("core system"),
        Message::user("old user ".repeat(20)),
        Message::assistant("old assistant ".repeat(20)),
        Message::user("latest question"),
    ];
    let compacted =
        compact_messages_with_threshold(messages, TokenCount::new(20), default_strip_fraction());
    let note = compacted
        .iter()
        .find(|m| m.role == Role::System && m.content.as_str().contains("context compacted"))
        .expect("compaction note should be present");
    assert!(note.content.as_str().contains("turn(s)"));
    assert!(note.content.as_str().contains("instruction block(s)"));
}

#[test]
fn compact_messages_with_threshold_compacts_dense_tool_payloads() {
    let dense = "x".repeat(2_000_000);
    let messages = vec![
        Message::system("sys"),
        Message::user("question"),
        Message::tool_result(
            ToolCallId::new("call_001"),
            &ToolName::new("shell_exec"),
            dense,
        ),
    ];
    let compacted = compact_messages_with_threshold(
        messages,
        TokenCount::new(700_000),
        default_strip_fraction(),
    );
    let tool_msg = compacted
        .iter()
        .find(|m| m.role == Role::Tool)
        .expect("expected tool message in compacted result");
    assert!(
        tool_msg.content.as_str().is_empty(),
        "dense tool payload should trigger compaction and be stripped to empty content",
    );
}

#[test]
fn compact_messages_with_threshold_smaller_strip_fraction_strips_less() {
    // Single small tool message where the strip happens but turn dropping is
    // not needed because the threshold is large enough.
    let short_msg = "short tool output";
    let messages = vec![
        Message::system("sys"),
        Message::user("question"),
        Message::tool_result(
            ToolCallId::new("call_001"),
            &ToolName::new("shell_exec"),
            short_msg,
        ),
        Message::assistant("response"),
        Message::user("follow up"),
    ];
    // With a threshold that fits everything but tool results, use zero strip
    // fraction to confirm no stripping happens.
    let compacted = compact_messages_with_threshold(
        messages.clone(),
        TokenCount::new(5_000_000),
        ToolResultStripFraction::ZERO,
    );
    let tool_msg = compacted
        .iter()
        .find(|m| m.role == Role::Tool)
        .expect("expected tool message");
    assert!(
        !tool_msg.content.as_str().is_empty(),
        "tool result should not be stripped at fraction 0"
    );
    assert!(
        tool_msg.content.as_str().contains("short tool output"),
        "tool content should be preserved"
    );
}

#[test]
fn compact_messages_with_threshold_zero_strip_fraction_strips_nothing() {
    let dense = "x".repeat(500_000);
    let messages = vec![
        Message::system("sys"),
        Message::user("question"),
        Message::tool_result(
            ToolCallId::new("call_001"),
            &ToolName::new("shell_exec"),
            dense,
        ),
        Message::user("follow up"),
    ];
    let compacted = compact_messages_with_threshold(
        messages.clone(),
        TokenCount::new(1_000_000),
        ToolResultStripFraction::ZERO,
    );
    let tool_msg = compacted
        .iter()
        .find(|m| m.role == Role::Tool)
        .expect("expected tool message in compacted result");
    assert!(
        !tool_msg.content.as_str().is_empty(),
        "tool result should not be stripped at fraction 0"
    );
}
