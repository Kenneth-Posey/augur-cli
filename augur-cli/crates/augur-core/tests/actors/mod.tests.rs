use std::fs;

#[test]
fn actors_mod_exports_supervisor_and_orchestrator_modules() {
    let source = fs::read_to_string(format!("{}/src/actors/mod.rs", env!("CARGO_MANIFEST_DIR")))
        .expect("actors mod source must be readable");

    assert!(source.contains("pub mod orchestrator;"));
    assert!(source.contains("pub mod supervisor;"));
    assert!(source.contains("pub use supervisor::SupervisorHandle;"));
}
