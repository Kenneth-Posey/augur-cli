#![allow(dead_code, unused_imports)]

//! TUI provider: terminal user interface implementation using Ratatui and Crossterm.
//!
//! Contains all TUI-specific actors, rendering components, domain types, and layout logic.
//! The TUI crate depends only on core domain types and actor handles; it has no dependencies
//! on provider SDKs (OpenRouter, Copilot, etc.).

extern crate self as augur_tui;

// ── Test discovery stubs (rust-analyzer visibility) ────────────────────────
// Makes VS Code / rust-analyzer discover tests in the external tests/
// directory.  Without these, tests declared only through [[test]] in
// Cargo.toml are invisible to the IDE test explorer.
#[cfg(test)]
#[path = "../tests/actors/mod.tests.rs"]
mod actors_tests;
#[cfg(test)]
#[path = "../tests/domain/mod.tests.rs"]
mod domain_tests;
#[cfg(test)]
#[path = "../tests/tui/mod.tests.rs"]
mod tui_tests;

/// TUI actor implementations and actor-specific helpers.
pub mod actors;
/// TUI domain types: state machines, input classifiers, render utilities.
// Test-compat re-exports: mirrored test modules reference types from
// augur-core/augur-domain via crate::config, crate::tools, etc.
#[cfg(test)]
pub(crate) mod config {
    pub use augur_domain::config::*;
}
#[cfg(test)]
pub(crate) mod tools {
    pub use augur_core::tools::*;
}
pub mod domain;
/// Rendering utilities: layout, components, screens, widgets.
pub mod tui;

pub use tui::layout;

// Re-export commonly used public types for convenience
pub use actors::tui::handle::TuiHandle;
pub use actors::tui::tui_actor::{TuiServiceTools, TuiSubActorHandles};
pub use domain::{
    tui_display_state::TuiDisplayState,
    tui_state::{AppScreen, AppState, ConversationMode},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Provider marker exposed by the TUI crate.
pub struct UiProviderName(&'static str);

impl std::fmt::Display for UiProviderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Return the provider marker for this crate.
pub fn provider() -> UiProviderName {
    UiProviderName("tui")
}
