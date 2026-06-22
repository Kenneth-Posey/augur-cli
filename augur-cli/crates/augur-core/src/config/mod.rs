//! Configuration module: types and YAML loader.
//!
//! Loader modules are core-owned. Domain config types are imported
//! directly from `augur-domain`.

pub mod endpoint_catalog_discovery;
pub mod loader;
pub mod program_settings;
pub mod provider_catalog;
pub mod user_settings;

mod write_section;

pub use loader::load_config;
pub use program_settings::{
    ProgramSettings, load_program_settings, save_program_settings, save_program_settings_sync,
};
pub use user_settings::{
    UserSettings, load_user_settings, save_user_settings, save_user_settings_sync,
};
pub(crate) use write_section::write_section_value;
