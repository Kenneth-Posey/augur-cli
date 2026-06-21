use augur_core::actors::user_message_consumer::user_message_consumer_ops::UserMessageCmd;
use std::fs;

#[test]
fn ops_source_contains_parse_user_input_contract() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/user_message_consumer/user_message_consumer_ops.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("user_message_consumer_ops source must be readable");

    assert!(source.contains("fn parse_user_input("));
    assert!(source.contains("UserInputTag::ParsedCommand"));
    assert!(source.contains("UserInputTag::RawCommand"));
}

#[test]
fn user_message_cmd_variants_are_available() {
    let process = UserMessageCmd::ProcessInput("hello".to_owned());
    match process {
        UserMessageCmd::ProcessInput(text) => assert_eq!(text, "hello"),
        UserMessageCmd::Shutdown => panic!("expected ProcessInput"),
    }

    let shutdown = UserMessageCmd::Shutdown;
    assert!(matches!(shutdown, UserMessageCmd::Shutdown));
}
