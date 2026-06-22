#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Provider marker exposed by the integration-tests crate.
pub struct IntegrationTestMarker;

/// Return the provider marker for this crate.
pub fn integration_test_marker() -> IntegrationTestMarker {
    IntegrationTestMarker
}
