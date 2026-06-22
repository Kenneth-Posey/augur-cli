//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Tool abstraction layer: definitions, handlers, registry, and built-in tools.

/// Bundled built-in tool implementations (file I/O, shell exec, etc.).
pub mod builtin;
/// Shared tool-execution normalization helpers.
pub mod execution;
/// Dispatch handler: routes an incoming tool call to its registered implementation.
pub mod handler;
/// Lower-tier provider contracts used by tool implementations.
pub(crate) mod ports;
/// Tool registry: registration, lookup, and lifecycle for all tools in this process.
///
/// Runtime wiring registers built-ins (including `size_check`) into this registry.
pub mod registry;

pub use augur_domain::tools::definition::*;
