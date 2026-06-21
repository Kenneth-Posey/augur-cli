use augur_core::actors::file_scanner::parse_file_attachments;
use augur_domain::domain::string_newtypes::PromptText;

#[test]
fn parse_file_attachments_splits_prompt_and_paths() {
    let input = PromptText::new("hello @src/main.rs world @Cargo.toml");
    let (clean, attachments) = parse_file_attachments(&input);

    assert_eq!(clean.as_str(), "hello world");
    assert_eq!(attachments.len(), 2);
    assert_eq!(attachments[0].as_str(), "src/main.rs");
    assert_eq!(attachments[1].as_str(), "Cargo.toml");
}
