use augur_provider_copilot_sdk::actors::copilot::assistant::context_ops::{
    format_sdk_error, log_sdk_error,
};

#[test]
fn mirrored_surface_smoke_context_ops() {
    let function_name = core::any::type_name_of_val(&format_sdk_error);
    assert!(function_name.contains("format_sdk_error"));
    let function_name = core::any::type_name_of_val(&log_sdk_error);
    assert!(function_name.contains("log_sdk_error"));
}
