use augur_cli::wiring::{await_runtime, shutdown_runtime};

/// Verifies the mirrored unit-test module can reach this file's surface symbols.
#[test]
fn mirrored_surface_smoke_lifecycle() {
    let function_name = core::any::type_name_of_val(&shutdown_runtime);
    assert!(function_name.contains("shutdown_runtime"));
    let function_name = core::any::type_name_of_val(&await_runtime);
    assert!(function_name.contains("await_runtime"));
}
