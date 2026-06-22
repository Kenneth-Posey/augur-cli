//! Status bar construction: git branch, model display, and project token snapshot.

use crate::domain::tui_state::StatusBarData;
use crate::domain::tui_status::refresh_status_bar_base_fields;
use augur_domain::config::types::AppConfig;
use augur_domain::config::types::find_endpoint;
use augur_domain::domain::EffortLevel;
use augur_domain::domain::string_newtypes::{EndpointName, ModelLabel, StringNewtype, WorkingDir};

/// Fallback model display label used when Copilot SDK manages the model internally.
const COPILOT_FALLBACK_LABEL: &str = "copilot";

/// Build the status bar data model from config and endpoint.
///
/// Reads the current working directory, runs `read_git_branch`, and formats
/// the model display string. Called once in `run()` before the event loop starts.
///
/// Consumers: `run` in `actor.rs` during TUI actor initialization.
pub(crate) fn build_status_bar(config: &AppConfig, ep_name: &EndpointName) -> StatusBarData {
    let mut status = StatusBarData::builder()
        .model_display(format_model_display(config, ep_name))
        .cwd(WorkingDir::new(""))
        .context_window(Default::default())
        .build();
    refresh_status_bar_base_fields(&mut status);
    status
}

/// Format the model display string as `"{model} ({effort})"` for the status bar.
///
/// When `config.copilot.copilot_chat.enabled` is true, returns a Copilot-specific label
/// using the configured model name (or `"copilot"` as a fallback) without an
/// effort suffix, since the Copilot SDK manages effort internally.
///
/// For all other endpoints, looks up the endpoint by name in `config.endpoints`
/// to retrieve the model identifier. Falls back to the raw endpoint name when the
/// endpoint is not found. Derives the effort label from `config.agent.temperature`
/// via `EffortLevel::from_temperature`. Used in `build_status_bar` and testable
/// independently.
///
/// Consumers: `build_status_bar` in this module; tests for model label formatting.
pub fn format_model_display(config: &AppConfig, ep_name: &EndpointName) -> ModelLabel {
    if let Some(endpoint) = find_endpoint(config, ep_name) {
        let effort = EffortLevel::from_temperature(config.agent.temperature);
        return ModelLabel::new(format!("{} ({})", endpoint.model, effort.label()));
    }
    if config.copilot.copilot_chat.enabled.0 {
        let model = config
            .copilot
            .copilot_chat
            .sdk
            .model
            .as_ref()
            .map(|model| model.as_str())
            .unwrap_or(COPILOT_FALLBACK_LABEL);
        return ModelLabel::new(model);
    }
    let model = ep_name.as_str().to_owned();
    let effort = EffortLevel::from_temperature(config.agent.temperature);
    ModelLabel::new(format!("{} ({})", model, effort.label()))
}
