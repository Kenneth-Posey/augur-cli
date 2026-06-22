#![allow(dead_code, unused_imports)]

//! TUI provider: terminal user interface implementation using Ratatui and Crossterm.
//!
//! Contains all TUI-specific actors, rendering components, domain types, and layout logic.
//! The TUI crate depends only on core domain types and actor handles; it has no dependencies
//! on provider SDKs (OpenRouter, Copilot, etc.).

/// TUI actor implementations and actor-specific helpers.
pub mod actors;
/// TUI domain types: state machines, input classifiers, render utilities.
pub mod domain;
/// Rendering utilities: layout, components, screens, widgets.
pub mod tui;

// Re-export modules for direct access
pub use tui::layout;
pub use tui::plan_panel;

// Re-export commonly used public types for convenience
pub use actors::tui::handle::TuiHandle;
pub use actors::tui::tui_actor::{TuiServiceTools, TuiSubActorHandles};
pub use domain::{
    tui_display_state::TuiDisplayState,
    tui_state::{AppScreen, AppState, ConversationMode},
};
pub use tui::layout::{compute_plan_layout, PLAN_PANEL_WIDTH_PERCENT};
pub use tui::plan_panel::{render_plan_panel, PlanPanelRender};

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
