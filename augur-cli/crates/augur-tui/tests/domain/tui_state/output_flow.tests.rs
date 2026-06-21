use super::*;

/// Verifies the mirrored unit-test module can reach this file's surface symbols.
#[test]
fn mirrored_surface_smoke_output_flow() {
    let function_name = core::any::type_name_of_val(&last_line_prevents_append);
    assert!(function_name.contains("last_line_prevents_append"));
    let function_name = core::any::type_name_of_val(&build_header_from_pending_response);
    assert!(function_name.contains("build_header_from_pending_response"));
}
