//! No direct `*.tests.rs` mirror by design: this module is a facade/re-export layer.
//! Behavior is validated by mirrored tests of child modules and higher-level integration tests.
//! Catalog manager actor: generates model catalogs from provider APIs.
//!
//! Queries one or more provider APIs for available language models and produces
//! YAML or Markdown output suitable for configuration or documentation.

pub mod catalog_manager_actor;
mod catalog_manager_actor_ops;
pub mod handle;
pub mod models;

pub use handle::CatalogManagerHandle;
