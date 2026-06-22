use augur_core::actors::command::types::{CommandDef, CommandOutcome};

#[test]
fn command_def_reexport_supports_builder() {
    let command = CommandDef::builder()
        .name("ping")
        .usage("/ping")
        .description("Ping the application")
        .build();
    assert_eq!(command.name, "ping");
    assert_eq!(command.usage, "/ping");
}

#[test]
fn command_outcome_reexport_exposes_variants() {
    let outcome = CommandOutcome::Quit;
    assert!(matches!(outcome, CommandOutcome::Quit));
}
