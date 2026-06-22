use augur_core::actors::command::command_actor::build;
use augur_core::actors::command::types::CommandOutcome;
use augur_domain::domain::string_newtypes::{PromptText, StringNewtype};

#[test]
fn build_returns_handle_with_builtins() {
    let handle = build(&[]);
    assert!(!handle.all_commands().is_empty());
    assert!(handle.all_commands().iter().any(|cmd| cmd.name == "help"));
}

#[test]
fn handle_executes_ping_command() {
    let handle = build(&[]);
    match handle.execute(&PromptText::from("/ping")) {
        CommandOutcome::SystemMessage(message) => assert_eq!(message.as_str(), "[system] pong"),
        _ => panic!("expected /ping to produce CommandOutcome::SystemMessage"),
    }
}
