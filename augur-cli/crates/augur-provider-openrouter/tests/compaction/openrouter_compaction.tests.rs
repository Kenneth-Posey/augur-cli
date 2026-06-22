use augur_domain::domain::newtypes::{TokenCount, ToolResultStripFraction};
use augur_domain::domain::string_newtypes::StringNewtype;
use augur_domain::domain::types::{Message, Role};
use augur_domain::NumericNewtype;

use augur_provider_openrouter::compaction::compact_messages_for_openrouter;

fn default_threshold() -> TokenCount {
    TokenCount::of(400_000)
}

fn default_strip_fraction() -> ToolResultStripFraction {
    ToolResultStripFraction::new(0.9)
}

#[test]
fn compact_messages_for_openrouter_preserves_order_when_under_threshold() {
    let messages = vec![
        Message::system("prefix"),
        Message::system("system"),
        Message::user("question"),
    ];
    let compacted = compact_messages_for_openrouter(
        messages.clone(),
        default_threshold(),
        default_strip_fraction(),
    );
    assert_eq!(compacted.len(), messages.len());
    assert_eq!(compacted[0].role, Role::System);
    assert_eq!(compacted[0].content.as_str(), "prefix");
    assert_eq!(compacted[1].content.as_str(), "system");
}

#[test]
fn compact_messages_for_openrouter_handles_empty_input() {
    let compacted =
        compact_messages_for_openrouter(Vec::new(), default_threshold(), default_strip_fraction());
    assert!(compacted.is_empty());
}
