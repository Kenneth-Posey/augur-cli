use augur_cli::wiring::{spawn_supervisor_runtime, wire_supervisor};

#[test]
fn mirrored_surface_smoke_supervisor() {
    let function_name = core::any::type_name_of_val(&wire_supervisor);
    assert!(function_name.contains("wire_supervisor"));
    let function_name = core::any::type_name_of_val(&spawn_supervisor_runtime);
    assert!(function_name.contains("spawn_supervisor_runtime"));
}
