use augur_domain::domain::string_newtypes::StringNewtype;
use augur_domain::domain::thinking_mode::ReasoningEffort;

#[test]
fn parse_optional_all_known_variants() {
    assert_eq!(
        ReasoningEffort::parse_optional("auto"),
        Some(ReasoningEffort::Auto)
    );
    assert_eq!(
        ReasoningEffort::parse_optional("high"),
        Some(ReasoningEffort::High)
    );
    assert_eq!(
        ReasoningEffort::parse_optional("medium"),
        Some(ReasoningEffort::Medium)
    );
    assert_eq!(
        ReasoningEffort::parse_optional("low"),
        Some(ReasoningEffort::Low)
    );
    assert_eq!(
        ReasoningEffort::parse_optional("none"),
        Some(ReasoningEffort::None)
    );
}

#[test]
fn parse_optional_unknown_returns_none() {
    assert_eq!(ReasoningEffort::parse_optional("turbo"), Option::None);
}

#[test]
fn options_contains_all_five_variants() {
    assert_eq!(ReasoningEffort::options().len(), 5);
}

#[test]
fn display_label_auto_contains_recommended() {
    let label = ReasoningEffort::Auto.display_label();
    assert!(label.as_str().contains("recommended"));
}
