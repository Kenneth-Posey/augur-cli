#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Provider marker exposed by the integration-tests crate.
pub struct IntegrationTestMarker;

// ── Test discovery stubs (rust-analyzer visibility) ────────────────────────
// Makes VS Code / rust-analyzer discover tests in the external tests/
// directory.  Without these, tests declared only through [[test]] in
// Cargo.toml are invisible to the IDE test explorer.
#[cfg(test)]
#[path = "../tests/integration/executor_permissions.tests.rs"]
mod executor_permissions_tests;
#[cfg(test)]
#[path = "../tests/integration_full_turn.tests.rs"]
mod integration_full_turn_tests;
#[cfg(test)]
#[path = "../tests/integration/llm_openrouter.tests.rs"]
mod llm_openrouter_tests;
#[cfg(test)]
#[path = "../tests/r3_2_snapshot_testing.tests.rs"]
mod r3_2_snapshot_testing_tests;
#[cfg(test)]
#[path = "../tests/workspace_smoke.tests.rs"]
mod workspace_smoke_tests;
/// Return the provider marker for this crate.
pub fn integration_test_marker() -> IntegrationTestMarker {
    IntegrationTestMarker
}
