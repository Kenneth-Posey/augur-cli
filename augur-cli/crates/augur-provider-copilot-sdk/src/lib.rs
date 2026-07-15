//! Copilot-provider workspace crate for shared helpers.

extern crate self as augur_provider_copilot_sdk;

/// Actor implementations owned by the Copilot provider crate.
pub mod actors;
/// Shared Copilot session and permission helpers.
pub mod shared;

// ── Test discovery stubs (rust-analyzer visibility) ────────────────────────
#[cfg(test)]
#[path = "../tests/actors/mod.tests.rs"]
mod actors_tests;
#[cfg(test)]
#[path = "../tests/shared/mod.tests.rs"]
mod shared_tests;
