use augur_domain::domain::string_newtypes::{EndpointName, ModelId, PromptText, StringNewtype};

#[test]
fn endpoint_name_roundtrip() {
    let name = EndpointName::new("openrouter");
    assert_eq!(name.as_str(), "openrouter");
    assert_eq!(name.into_inner(), "openrouter");
}

#[test]
fn model_id_equality() {
    let a = ModelId::new("gpt-4o");
    let b = ModelId::new("gpt-4o");
    assert_eq!(a, b);
}

#[test]
fn prompt_text_display() {
    let pt = PromptText::new("hello world");
    assert_eq!(pt.to_string(), "hello world");
}

#[test]
fn model_id_different_values_not_equal() {
    let a = ModelId::new("gpt-4o");
    let b = ModelId::new("claude-3");
    assert_ne!(a, b);
}
