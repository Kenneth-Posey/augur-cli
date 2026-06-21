use augur_domain::domain::newtypes::{NumericNewtype, TokenCount};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::domain::types::{Message, Role};
use augur_domain::newtypes::ToolResultStripFraction;
use augur_provider_openrouter::compaction::{
    compact_messages_with_threshold, estimate_text_tokens,
};

fn default_strip_fraction() -> ToolResultStripFraction {
    ToolResultStripFraction::new(0.9)
}

#[test]
fn estimate_text_tokens_uses_the_larger_of_word_and_character_counts() {
    let short = estimate_text_tokens(&OutputText::new("two words"));
    let longer = estimate_text_tokens(&OutputText::new("abcdefghij"));

    assert!(longer >= short);
    assert!(short.inner() >= 2);
}

#[test]
fn compact_messages_with_large_threshold_keeps_the_input_unchanged() {
    let messages = vec![
        Message::system("keep system"),
        Message::user("first user turn"),
        Message::assistant("first assistant turn"),
    ];

    let compacted = compact_messages_with_threshold(
        messages.clone(),
        TokenCount::new(u64::MAX),
        default_strip_fraction(),
    );

    assert_eq!(compacted.len(), messages.len());
    for (left, right) in compacted.iter().zip(messages.iter()) {
        assert_eq!(left.role, right.role);
        assert_eq!(left.content.as_str(), right.content.as_str());
    }
}

#[test]
fn compact_messages_with_tiny_threshold_preserves_the_system_prompt() {
    let messages = vec![
        Message::system("keep system"),
        Message::user("first user turn that is long enough to trigger compaction"),
        Message::assistant("first assistant turn that is long enough to trigger compaction"),
        Message::user("second user turn that is long enough to trigger compaction"),
    ];

    let compacted =
        compact_messages_with_threshold(messages, TokenCount::new(1), default_strip_fraction());

    assert!(!compacted.is_empty());
    assert_eq!(compacted[0].role, Role::System);
}
