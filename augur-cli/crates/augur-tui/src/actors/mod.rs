//! TUI actor implementations: main TUI actor and specialized panel actors.

// Test-compat re-exports: mirrored test modules reference types from augur-core
// and augur-domain via crate::actors::<name> and crate::domain::<name> paths.
// These forward references keep the tests compileable without rewriting imports
// across dozens of test files.
#[cfg(test)]
pub(crate) mod agent {
    pub use augur_core::actors::agent::*;
}
#[cfg(test)]
pub(crate) mod session {
    pub use augur_core::actors::session::*;
}
#[cfg(test)]
pub(crate) mod command {
    pub use augur_core::actors::command::*;
}
#[cfg(test)]
pub(crate) mod file_scanner {
    pub use augur_core::actors::file_scanner::*;
}
#[cfg(test)]
pub(crate) mod llm {
    pub use augur_core::actors::llm_feed_consumer::*;
}
#[cfg(test)]
pub(crate) mod ask {
    pub use augur_core::actors::ask::*;
}
#[cfg(test)]
pub(crate) mod logger {
    pub use augur_core::actors::logger::*;
}
#[cfg(test)]
pub(crate) mod token_tracker {
    pub use augur_core::actors::token_tracker::*;
}
pub(crate) mod catalog_manager {
    pub use augur_core::actors::catalog_manager::*;
}
pub mod tui;
pub mod tui_agent_panel;
pub mod tui_ask_panel;
pub mod tui_chat_menu;
pub mod tui_dynamic_controls;
pub mod tui_main_feed_panel;
pub mod tui_spinner;

pub use tui::handle::TuiHandle;
pub use tui::tui_actor::{TuiServiceTools, TuiSubActorHandles};
pub use tui_agent_panel::TuiAgentPanelHandle;
pub use tui_ask_panel::TuiAskPanelHandle;
pub use tui_chat_menu::TuiChatMenuHandle;
pub use tui_dynamic_controls::TuiDynamicControlsHandle;
pub use tui_main_feed_panel::TuiMainFeedPanelHandle;
pub use tui_spinner::TuiSpinnerHandle;
