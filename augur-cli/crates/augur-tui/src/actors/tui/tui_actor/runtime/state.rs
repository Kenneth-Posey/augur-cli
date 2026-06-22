//! Startup-state construction helpers for the TUI runtime.

use crate::actors::tui::assistant::status_bar::build_status_bar;
use crate::domain::tui_state::{
    AppScreen, AppState, PickerSessionIdentity, PickerSessionSummary, PickerState,
};
use augur_domain::domain::newtypes::Count;
use augur_domain::domain::string_newtypes::StringNewtype;

/// Build the initial TUI state from startup data and actor handles.
pub(super) fn build_initial_state(
    providers: &super::super::TuiServiceHandles,
    startup: &super::super::TuiStartupData,
) -> AppState {
    let default_ep = providers.session.active_endpoint();
    let mode = build_initial_mode(startup.session_summaries.clone());
    let mut state = AppState::new(default_ep.clone(), mode);
    configure_model_catalog(&mut state, startup, &default_ep);
    if matches!(state.interaction.screen, AppScreen::Conversation) {
        providers.agent.replace_session(None);
    }
    state.status = build_status_bar(&startup.config, &default_ep);
    apply_saved_model_display(&mut state);
    state
}

fn configure_model_catalog(
    state: &mut AppState,
    startup: &super::super::TuiStartupData,
    default_ep: &augur_domain::domain::string_newtypes::EndpointName,
) {
    state.prompt.models.endpoint_catalog =
        augur_core::config::endpoint_catalog_discovery::discover_endpoint_catalog(&startup.config);
    let Some(row) = state
        .prompt
        .models
        .endpoint_catalog
        .iter()
        .find(|row| row.endpoint_name == *default_ep)
    else {
        return;
    };
    state.prompt.models.available = row.models.clone();
    state.status.model_display = row.default_display.clone();
    if row.supports_auto.into() {
        state.prompt.models.active_id =
            Some(augur_domain::domain::string_newtypes::ModelId::new(""));
    }
}

fn apply_saved_model_display(state: &mut AppState) {
    let user_settings = augur_core::config::user_settings::load_user_settings();
    let Some(model_str) = &user_settings.last_model else {
        return;
    };
    use augur_domain::domain::string_newtypes::{ModelId, StringNewtype};
    let model_id = ModelId::new(model_str.as_str());
    let model_is_available = state
        .prompt
        .models
        .available
        .iter()
        .any(|model| model.id == model_id);
    if model_is_available {
        state.status.model_display = model_str.as_str().into();
        state.prompt.models.active_id = Some(model_id);
    }
}

fn build_initial_mode(
    summaries: Vec<augur_domain::persistence::types::SessionSummary>,
) -> AppScreen {
    if summaries.is_empty() {
        AppScreen::Conversation
    } else {
        AppScreen::SessionSelector(PickerState {
            sessions: summaries.into_iter().map(into_picker_session).collect(),
            selected: Count::of(0),
        })
    }
}

fn into_picker_session(
    summary: augur_domain::persistence::types::SessionSummary,
) -> PickerSessionSummary {
    PickerSessionSummary::builder()
        .identity(
            PickerSessionIdentity::builder()
                .id(summary.identity.id)
                .created_at(summary.identity.created_at)
                .last_updated_at(summary.identity.last_updated_at)
                .endpoint_name(summary.identity.endpoint_name)
                .build(),
        )
        .message_count(summary.message_count)
        .preview(summary.preview)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::config::types::{
        AgentConfig, AppConfig, CopilotConfig, EndpointConfig, EndpointCredentials,
        PersistenceConfig, Provider,
    };
    use augur_domain::domain::TokenCount;
    use augur_domain::domain::newtypes::{NumericNewtype, Temperature};
    use augur_domain::domain::string_newtypes::{
        EndpointName, EndpointUrl, FilePath, ModelName, OutputText, StringNewtype,
    };
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    const PROVIDER_DIR_ENV: &str = "AUGUR_CLI_PROVIDER_CATALOG_DIR";

    fn provider_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn test_config() -> AppConfig {
        AppConfig {
            endpoints: vec![EndpointConfig {
                name: EndpointName::new("primary"),
                provider: Provider::OpenAi,
                base_url: EndpointUrl::new("https://api.openai.com/v1"),
                model: ModelName::new("fallback-model"),
                credentials: EndpointCredentials::default(),
            }],
            default_endpoint: EndpointName::new("primary"),
            agent: AgentConfig {
                system_prompt: OutputText::new("test"),
                max_tokens: TokenCount::new(256),
                temperature: Temperature::new(0.2),
                allowed_dirs: vec![FilePath::new("./")],
            },
            copilot: CopilotConfig::default(),
            persistence: PersistenceConfig {
                log_dir: FilePath::new("./logs"),
                sessions_dir: None,
            },
            program_settings: Default::default(),
            user_settings: Default::default(),
        }
    }

    fn write_provider_catalog(provider_dir: &std::path::Path) {
        std::fs::write(
            provider_dir.join("openai.yaml"),
            r#"provider: openai
models:
  - id: gpt-replacement
    display_name: GPT Replacement
    cost_input_per_mtok: 1.0
    cost_output_per_mtok: 2.0
"#,
        )
        .expect("write provider catalog");
    }

    fn restore_provider_dir(previous: Option<OsString>) {
        match previous {
            // TODO: Audit that the environment access only happens in single-threaded code.
            Some(value) => unsafe { std::env::set_var(PROVIDER_DIR_ENV, value) },
            // TODO: Audit that the environment access only happens in single-threaded code.
            None => unsafe { std::env::remove_var(PROVIDER_DIR_ENV) },
        }
    }

    #[tokio::test]
    async fn configure_model_catalog_uses_provider_catalog_runtime_path() {
        let _guard = provider_env_lock()
            .lock()
            .expect("provider env lock poisoned");
        let provider_dir = tempfile::tempdir().expect("provider tempdir");
        write_provider_catalog(provider_dir.path());
        let previous = std::env::var_os(PROVIDER_DIR_ENV);
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::set_var(PROVIDER_DIR_ENV, provider_dir.path()) };

        let config = test_config();
        let mut state = AppState::new(EndpointName::new("primary"), AppScreen::Conversation);
        let startup = crate::actors::tui::tui_actor::TuiStartupData::builder()
            .session_summaries(vec![])
            .persistence(augur_domain::persistence::handle::PersistenceHandle::new(
                tempfile::tempdir()
                    .expect("persistence tempdir")
                    .path()
                    .to_path_buf(),
            ))
            .token_tracker(augur_core::actors::token_tracker::token_tracker_actor::spawn().1)
            .config(config)
            .renderer(crate::tui::render::render_with_overlays)
            .build();
        configure_model_catalog(&mut state, &startup, &EndpointName::new("primary"));
        assert_eq!(state.prompt.models.available.len(), 1);
        assert_eq!(
            state.prompt.models.available[0].id.as_str(),
            "gpt-replacement"
        );
        assert_ne!(
            state.prompt.models.available[0].id.as_str(),
            "fallback-model"
        );
        restore_provider_dir(previous);
    }
}
