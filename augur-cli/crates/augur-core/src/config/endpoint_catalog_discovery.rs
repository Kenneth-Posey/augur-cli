//! Endpoint discovery for the LLM actor startup model menu.
//!
//! Reads `AppConfig.endpoints` and converts each entry to a `ModelOption` so
//! the TUI `/model` picker can list every configured LLM endpoint at startup.
use augur_domain::config::provider_catalog::{
    default_provider_catalog_dir, load_provider_catalog, provider_catalog_path,
};
use augur_domain::config::types::{AppConfig, EndpointConfig};
use augur_domain::domain::endpoint_model_catalog::EndpointModelCatalog;
use augur_domain::domain::newtypes::SupportsAuto;
use augur_domain::domain::string_newtypes::{EndpointName, ModelId, ModelLabel, StringNewtype};
use augur_domain::domain::types::ModelOption;
use augur_domain::domain::EffortLevel;
use std::path::Path;
/// Build the startup `/model` list from endpoint catalogs.
///
/// Uses provider YAML catalogs (`configs/providers/*.yaml`) when available, with
/// endpoint-model fallback handled by `discover_endpoint_catalog`.
pub fn discover_endpoints(config: &AppConfig) -> Vec<ModelOption> {
    config
        .endpoints
        .iter()
        .map(startup_model_option_for_endpoint)
        .collect()
}
/// Build per-endpoint model catalogs for `/switch` model refresh.
pub fn discover_endpoint_catalog(config: &AppConfig) -> Vec<EndpointModelCatalog> {
    let provider_dir = default_provider_catalog_dir();
    discover_endpoint_catalog_for_provider_dir(config, provider_dir.as_path())
}
/// Testable variant of [`discover_endpoint_catalog`] that accepts an explicit provider directory.
///
/// Behaves identically to [`discover_endpoint_catalog`] but reads per-provider YAML
/// catalog files from `provider_dir` instead of [`default_provider_catalog_dir()`].
/// This separation allows tests to supply a temporary directory without touching the
/// global default path.
///
/// `config` provides the endpoint list and copilot settings used to build the catalog
/// rows. `provider_dir` is the directory that contains per-provider YAML files (e.g.
/// `openai.yaml`, `anthropic.yaml`).
///
/// Called by [`discover_endpoint_catalog`] and directly by tests.
pub fn discover_endpoint_catalog_for_provider_dir(
    config: &AppConfig,
    provider_dir: &Path,
) -> Vec<EndpointModelCatalog> {
    let effort = EffortLevel::from_temperature(config.agent.temperature);
    let mut rows: Vec<EndpointModelCatalog> = config
        .endpoints
        .iter()
        .map(|ep| build_endpoint_catalog_row(ep, provider_dir, effort))
        .collect();
    if config.copilot.copilot_chat.enabled.0 {
        let copilot_model = config
            .copilot
            .copilot_chat
            .sdk
            .model
            .as_ref()
            .map(|m| m.as_str().to_owned())
            .unwrap_or_else(|| "copilot".to_owned());
        rows.push(
            EndpointModelCatalog::builder()
                .endpoint_name(EndpointName::new("copilot"))
                .models(vec![])
                .default_display(ModelLabel::new(copilot_model))
                .supports_auto(SupportsAuto::yes())
                .build(),
        );
    }
    rows
}
fn startup_model_option_for_endpoint(ep: &EndpointConfig) -> ModelOption {
    ModelOption::builder()
        .id(ModelId::new(ep.name.as_str()))
        .display_name(ModelLabel::new(format!("{} ({})", ep.model, ep.provider)))
        .build()
}
fn build_endpoint_catalog_row(
    ep: &EndpointConfig,
    provider_dir: &Path,
    effort: EffortLevel,
) -> EndpointModelCatalog {
    let fallback_model = fallback_model_option(ep);
    let models = match provider_models_for_endpoint(ep, provider_dir) {
        ProviderModelsLoad::Loaded(models) => models,
        ProviderModelsLoad::Missing => vec![fallback_model],
        ProviderModelsLoad::Malformed(err) => {
            tracing::warn!(
                endpoint = %ep.name,
                provider = %ep.provider,
                error = %err,
                "malformed provider catalog; falling back to endpoint model"
            );
            vec![fallback_model]
        }
        ProviderModelsLoad::Unavailable(err) => {
            tracing::warn!(
                endpoint = %ep.name,
                provider = %ep.provider,
                error = %err,
                "provider catalog unavailable; keeping endpoint model list empty"
            );
            vec![]
        }
    };
    EndpointModelCatalog::builder()
        .endpoint_name(ep.name.clone())
        .models(models)
        .default_display(ModelLabel::new(format!(
            "{} ({})",
            ep.model,
            effort.label()
        )))
        .supports_auto(SupportsAuto::no())
        .build()
}
fn fallback_model_option(ep: &EndpointConfig) -> ModelOption {
    ModelOption::builder()
        .id(ModelId::new(ep.model.as_str()))
        .display_name(ModelLabel::new(format!("{} ({})", ep.model, ep.provider)))
        .build()
}
enum ProviderModelsLoad {
    Loaded(Vec<ModelOption>),
    Missing,
    Malformed(anyhow::Error),
    Unavailable(anyhow::Error),
}
fn provider_models_for_endpoint(ep: &EndpointConfig, provider_dir: &Path) -> ProviderModelsLoad {
    let catalog_path = provider_catalog_path(provider_dir, ep.provider.clone());
    if !catalog_path.exists() {
        return ProviderModelsLoad::Missing;
    }
    let maybe_catalog = match load_provider_catalog(provider_dir, ep.provider.clone()) {
        Ok(catalog) => catalog,
        Err(err) => {
            if is_malformed_catalog_error(&err) {
                return ProviderModelsLoad::Malformed(err);
            }
            return ProviderModelsLoad::Unavailable(err);
        }
    };
    let Some(catalog) = maybe_catalog else {
        return ProviderModelsLoad::Missing;
    };
    let mut models: Vec<ModelOption> = catalog
        .models
        .into_iter()
        .map(|model| {
            let display_name = model
                .display_name
                .unwrap_or_else(|| ModelLabel::new(model.id.as_str()));
            ModelOption::builder()
                .id(model.id)
                .display_name(display_name)
                .max_context_length(model.max_context_length)
                .tool_compaction_ratio(model.tool_compaction_ratio)
                .max_tool_iterations(model.max_tool_iterations)
                .compaction_target(model.compaction_target)
                .auto_compact_threshold(model.auto_compact_threshold)
                .build()
        })
        .collect();
    models.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    ProviderModelsLoad::Loaded(models)
}
fn is_malformed_catalog_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.contains("parsing provider catalog file")
        || msg.contains("declares provider")
        || msg.contains("missing field")
}
