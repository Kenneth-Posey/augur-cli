use augur_domain::domain::string_newtypes::StringNewtype;
use augur_domain::domain::thinking_mode::ReasoningEffort;

#[test]
fn parse_optional_roundtrips_all_variants() {
    let cases = [
        ("auto", ReasoningEffort::Auto),
        ("high", ReasoningEffort::High),
        ("medium", ReasoningEffort::Medium),
        ("low", ReasoningEffort::Low),
        ("none", ReasoningEffort::None),
    ];
    for (s, expected) in cases {
        assert_eq!(
            ReasoningEffort::parse_optional(s),
            Some(expected),
            "failed for {s}"
        );
    }
}

#[test]
fn parse_optional_returns_none_for_unknown() {
    assert_eq!(ReasoningEffort::parse_optional("unknown"), None);
    assert_eq!(ReasoningEffort::parse_optional(""), None);
}

#[test]
fn options_returns_five_variants() {
    assert_eq!(ReasoningEffort::options().len(), 5);
}

#[test]
fn display_label_contains_variant_name() {
    assert!(ReasoningEffort::Auto
        .display_label()
        .as_str()
        .contains("auto"));
    assert!(ReasoningEffort::High
        .display_label()
        .as_str()
        .contains("high"));
    assert!(ReasoningEffort::None
        .display_label()
        .as_str()
        .contains("none"));
}
