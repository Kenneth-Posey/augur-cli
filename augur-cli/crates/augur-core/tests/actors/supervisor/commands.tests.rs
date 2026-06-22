use std::fs;

#[test]
fn supervisor_commands_enum_contains_control_variants() {
    let source = fs::read_to_string(format!(
        "{}/src/actors/supervisor/commands.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("commands.rs must be readable");

    assert!(source.contains("pub enum SupervisorCmd"));
    assert!(source.contains("StartPlan"));
    assert!(source.contains("CancelPlan"));
    assert!(source.contains("InjectStep"));
}
