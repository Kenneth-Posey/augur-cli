#![allow(dead_code, unused_imports)]

extern crate self as augur_domain;

// ── Test discovery stubs (rust-analyzer visibility) ────────────────────────
// Makes VS Code / rust-analyzer discover tests in the external tests/
// directory.  Without these, tests declared only through [[test]] in
// Cargo.toml are invisible to the IDE test explorer.
#[cfg(test)]
#[path = "../tests/config/mod.tests.rs"]
mod config_tests;
#[cfg(test)]
#[path = "../tests/domain_tests.tests.rs"]
mod domain_integration_tests;
#[cfg(test)]
#[path = "../tests/domain/mod.tests.rs"]
mod domain_tests;
#[cfg(test)]
#[path = "../tests/persistence/mod.tests.rs"]
mod persistence_tests;
#[cfg(test)]
#[path = "../tests/tools/mod.tests.rs"]
mod tools_tests;

pub mod actors;
pub mod config;
pub mod domain;
pub mod persistence;
pub mod tools;

pub use actors::*;
pub use domain::*;
