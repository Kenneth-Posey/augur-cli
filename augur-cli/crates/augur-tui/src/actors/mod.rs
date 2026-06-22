//! TUI actor implementations: main TUI actor and specialized panel actors.

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
