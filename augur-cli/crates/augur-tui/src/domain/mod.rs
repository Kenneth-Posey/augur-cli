//! TUI-specific domain types: state machines, input classifiers, render utilities.

pub mod tui_display_state;
pub mod tui_input;
pub mod tui_render;
pub mod tui_state;
pub mod tui_status;

// Legacy test-compat re-exports used by mirrored TUI test modules.
pub mod newtypes {
    pub use augur_domain::domain::newtypes::*;
}
pub mod string_newtypes {
    pub use augur_domain::domain::string_newtypes::*;
}
pub mod types {
    pub use augur_domain::domain::types::*;
}

pub use tui_display_state::TuiDisplayState;
pub use tui_state::AppState;
// Legacy test-compat re-exports for mirrored test modules that reference
// types from augur-domain via crate::domain::<submodule> paths.
pub mod traits {
    pub use augur_domain::domain::traits::*;
}
pub mod thinking_mode {
    pub use augur_domain::domain::thinking_mode::*;
}
pub mod task_types {
    pub use augur_domain::domain::task_types::*;
}
pub use augur_core::domain::DeterministicOrchestratorEvent;
pub use augur_core::domain::NormalizedSignal;
pub use augur_domain::domain::string_newtypes::WorkflowStepId;
