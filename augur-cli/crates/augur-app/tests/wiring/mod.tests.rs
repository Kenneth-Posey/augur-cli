#![allow(unused_imports)]

pub use augur_cli::wiring::{
    build_spawned_tui_deps, build_tui_deps, build_tui_runtime_deps, spawn_app_runtime,
    spawn_chat_runtime, spawn_core_runtime, spawn_domain_actors, spawn_planning_actors,
    spawn_root_deterministic_orchestrator_runtime, spawn_supervisor_runtime, spawn_tui_actor,
    spawn_tui_runtime, spawn_tui_sub_actors, take_query_rx, wire_supervisor, AppRuntimeConfigRef,
    ConsumerHandles, CoreRuntime, DomainRuntimeConfigRef, EndpointRoutingChatProvider,
    SpawnedOptionalActors,
};

// NOTE: archived_wiring_tests module disabled - ../wiring.tests.rs file not found
// mod archived_wiring_tests {
//     include!("../wiring.tests.rs");
// }
