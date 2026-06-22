use augur_cli::wiring::{EndpointRoutingChatProvider, spawn_chat_runtime};

/// Verifies the mirrored unit-test module can reach this file's surface symbols.
#[test]
fn mirrored_surface_smoke_chat_provider() {
    let type_name = core::any::type_name::<EndpointRoutingChatProvider>();
    assert!(type_name.contains("EndpointRoutingChatProvider"));
    let function_name = core::any::type_name_of_val(&spawn_chat_runtime);
    assert!(function_name.contains("spawn_chat_runtime"));
}
