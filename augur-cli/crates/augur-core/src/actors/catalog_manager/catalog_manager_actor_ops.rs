//! Catalog manager functional core.

use super::handle::CatalogManagerCommand;
use super::models::fetchers;
use super::models::filter::filter_models;
use super::models::formatter::{to_markdown_catalog, to_yaml_snippet};
use super::models::{FilterOpts, ModelInfo, OutputFormat, ProviderChoice, ProviderName};
use crate::config::provider_catalog::{
    ProviderCatalogFile, ProviderCatalogModel, default_provider_catalog_dir, write_provider_catalog,
};
use augur_domain::domain::newtypes::{Count, NumericNewtype, TokenCount, ToolResultStripFraction};
use augur_domain::domain::string_newtypes::{ModelId, ModelLabel, OutputText, StringNewtype};

/// Main actor loop: receives and processes catalog generation commands.
pub(super) async fn run_actor(mut rx: tokio::sync::mpsc::Receiver<CatalogManagerCommand>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            CatalogManagerCommand::GenerateCatalog {
                provider_filter,
                format,
                tx,
            } => {
                let result = generate_catalog(provider_filter, format).await;
                let _ = tx.send(result);
            }
        }
    }
}

/// Fetch, filter, and persist model catalogs for one or all providers.
///
/// Derives a [`ProviderChoice`] from `provider_filter`, calls `fetch_all`, applies
/// default [`FilterOpts`], writes one YAML file per provider under
/// [`default_provider_catalog_dir()`], and returns a formatted summary string.
///
/// `provider_filter` selects a single provider by name (`"openai"`, `"anthropic"`,
/// `"openrouter"`, or `"ollama"`); pass `None` to fetch all providers in parallel.
/// `format` controls whether the returned summary is rendered as Markdown or YAML.
///
/// Returns an error if `provider_filter` contains an unrecognised provider name,
/// if a required API fetch fails (single-provider mode propagates the error directly;
/// multi-provider mode logs individual failures and continues), or if any provider
/// catalog file cannot be written to disk.
pub(super) async fn generate_catalog(
    provider_filter: Option<ProviderName>,
    format: OutputFormat,
) -> anyhow::Result<OutputText> {
    tracing::info!("generating catalog");

    let provider_choice = resolve_provider_choice(provider_filter.as_ref())?;

    let models = fetch_all(provider_choice).await?;
    tracing::info!("fetched {} models", models.len());

    let filter_opts = FilterOpts::builder().build();
    let filtered = filter_models(models, &filter_opts);
    tracing::info!("after filtering: {} models", filtered.len());

    let written_paths = persist_provider_catalogs(&filtered)?;
    tracing::info!("wrote {} provider catalog file(s)", written_paths.len());

    let output = format!(
        "# wrote {} provider catalog file(s) under {}\n{}",
        written_paths.len(),
        default_provider_catalog_dir().display(),
        format_output(&format, &filtered)
    );

    tracing::info!("catalog generation complete");
    Ok(OutputText::new(output))
}

async fn fetch_all(provider: ProviderChoice) -> anyhow::Result<Vec<ModelInfo>> {
    if matches!(provider, ProviderChoice::All) {
        fetch_from_all_providers().await
    } else {
        fetch_single_provider(provider).await
    }
}

fn resolve_provider_choice(
    provider_filter: Option<&ProviderName>,
) -> anyhow::Result<ProviderChoice> {
    provider_filter
        .map(|provider| parse_provider_choice(provider.0.as_str()))
        .transpose()
        .map(|choice| choice.unwrap_or(ProviderChoice::All))
}

fn parse_provider_choice(provider: &str) -> anyhow::Result<ProviderChoice> {
    if let Some(choice) = named_provider_choice(provider) {
        Ok(choice)
    } else {
        anyhow::bail!(
            "unknown provider: {}; use 'openai', 'anthropic', 'openrouter', 'ollama', or omit for all",
            provider
        )
    }
}

fn named_provider_choice(provider: &str) -> Option<ProviderChoice> {
    if provider == "openai" {
        Some(ProviderChoice::Openai)
    } else if provider == "anthropic" {
        Some(ProviderChoice::Anthropic)
    } else if provider == "openrouter" {
        Some(ProviderChoice::Openrouter)
    } else if provider == "ollama" {
        Some(ProviderChoice::Ollama)
    } else {
        None
    }
}

async fn fetch_single_provider(provider: ProviderChoice) -> anyhow::Result<Vec<ModelInfo>> {
    if matches!(provider, ProviderChoice::Openai) {
        fetchers::openai::fetch_models(None).await
    } else if matches!(provider, ProviderChoice::Anthropic) {
        fetchers::anthropic::fetch_models(None).await
    } else if matches!(provider, ProviderChoice::Openrouter) {
        Ok(vec![])
    } else if matches!(provider, ProviderChoice::Ollama) {
        fetchers::ollama::fetch_models().await
    } else {
        fetch_from_all_providers().await
    }
}

async fn fetch_from_all_providers() -> anyhow::Result<Vec<ModelInfo>> {
    let mut all: Vec<ModelInfo> = Vec::new();

    let results = tokio::join!(
        fetchers::openai::fetch_models(None),
        fetchers::anthropic::fetch_models(None),
        fetchers::ollama::fetch_models(),
    );

    for (name, result) in [
        ("openai", results.0),
        ("anthropic", results.1),
        ("ollama", results.2),
    ] {
        match result {
            Ok(models) => all.extend(models),
            Err(e) => tracing::warn!(provider = name, error = %e, "provider fetch failed"),
        }
    }

    Ok(all)
}

fn format_output(format: &OutputFormat, models: &[ModelInfo]) -> String {
    match format {
        OutputFormat::Markdown => to_markdown_catalog(models).0,
        OutputFormat::Yaml => to_yaml_snippet(models).0,
    }
}

fn persist_provider_catalogs(models: &[ModelInfo]) -> anyhow::Result<Vec<std::path::PathBuf>> {
    persist_provider_catalogs_in_dir(models, default_provider_catalog_dir().as_path())
}

fn persist_provider_catalogs_in_dir(
    models: &[ModelInfo],
    provider_dir: &std::path::Path,
) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut grouped: std::collections::BTreeMap<String, Vec<ProviderCatalogModel>> =
        std::collections::BTreeMap::new();
    for model in models {
        let display = if model.name.is_empty() {
            model.id.0.as_str()
        } else {
            model.name.as_str()
        };
        grouped
            .entry(model.provider.0.clone())
            .or_default()
            .push(ProviderCatalogModel {
                id: ModelId::new(model.id.0.as_str()),
                display_name: Some(ModelLabel::new(display)),
                cost_input_per_mtok: (*model.pricing.input_price_per_mtok).into(),
                cost_output_per_mtok: (*model.pricing.output_price_per_mtok).into(),
                supports_tools: Some(supports_tools(model.provider.0.as_str())),
                // Per-model config defaults: 0 means "use provider default".
                max_context_length: TokenCount::ZERO,
                tool_compaction_ratio: ToolResultStripFraction::ZERO,
                max_tool_iterations: Count::ZERO,
                compaction_target: TokenCount::ZERO,
                auto_compact_threshold: TokenCount::ZERO,
            });
    }

    grouped
        .into_iter()
        .map(|(provider, mut models)| {
            models.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            let file = ProviderCatalogFile {
                provider: provider.into(),
                models,
                openrouter: None,
            };
            write_provider_catalog(provider_dir, &file)
        })
        .collect()
}

fn supports_tools(provider: &str) -> bool {
    matches!(provider, "openai" | "anthropic" | "openrouter")
}
