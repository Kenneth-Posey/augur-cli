//! Per-model configuration resolution from provider catalog YAML files.
//!
//! Loads the OpenRouter provider catalog at runtime and extracts per-model
//! values for compaction target, strip fraction, max tool iterations, and
//! auto-compact threshold.
//! Every value falls back to a hardcoded default when the model is absent or
//! the field is set to its zero sentinel (meaning "use provider default").

use augur_domain::config::provider_catalog::{
    default_provider_catalog_dir, load_provider_catalog, ProviderCatalogFile,
};
use augur_domain::config::types::Provider;
use augur_domain::newtypes::{Count, NumericNewtype, TokenCount, ToolResultStripFraction};
use augur_domain::string_newtypes::ModelId;
use std::path::Path;

// ── Default values ────────────────────────────────────────────────────────────

/// Fallback compaction target when model config is absent or set to zero (400k tokens).
const FALLBACK_COMPACTION_TARGET: TokenCount = TokenCount::of(400_000);

/// Fallback max tool iterations when model config is absent or set to zero (100).
const FALLBACK_MAX_ITERATIONS: Count = Count::of(100);

/// Fallback auto-compact threshold when model config is absent or set to zero.
/// Defaults to 80% of the fallback compaction target (320_000 tokens).
const FALLBACK_AUTO_COMPACT_THRESHOLD: TokenCount = TokenCount::of(320_000);

// ── Public resolution API ─────────────────────────────────────────────────────

/// Per-model configuration values resolved from the provider catalog.
///
/// Every field is guaranteed to be populated with either the model-specific
/// value (when the model is found and the field is non-zero) or the hardcoded
/// fallback default.
#[derive(Clone, Debug)]
pub struct ResolvedModelConfig {
    /// Target token count after compaction. Compaction trims messages to this target.
    pub compaction_target: TokenCount,
    /// Maximum context length in tokens for the selected model (absolute max the model accepts).
    ///
    /// 0 means the provider catalog did not specify a value; consumers should fall back
    /// to a reasonable default at their call site.
    pub max_context_length: TokenCount,
    /// Fraction of oldest tool-result messages to strip during compaction.
    pub strip_fraction: ToolResultStripFraction,
    /// Maximum tool-call iterations before the task stops with a failure.
    pub max_iterations: Count,
    /// Token threshold that triggers automatic compaction toward compaction_target.
    pub auto_compact_threshold: TokenCount,
}

/// Resolve model configuration for an optional model ID.
///
/// When `model_id` is `Some`, loads the OpenRouter provider catalog and
/// searches for the matching model. Returns the model-specific values when
/// found and non-zero; falls back to compile-time defaults otherwise.
///
/// When `model_id` is `None`, returns defaults immediately without I/O.
pub fn resolve_model_config(model_id: Option<&ModelId>) -> ResolvedModelConfig {
    let Some(model_id) = model_id else {
        return fallback_config();
    };
    resolve_model_config_for_id(model_id)
}

fn resolve_model_config_for_id(model_id: &ModelId) -> ResolvedModelConfig {
    let provider_dir = default_provider_catalog_dir();
    match load_openrouter_catalog(provider_dir.as_path()) {
        Some(catalog) => config_from_catalog(&catalog, model_id),
        None => fallback_config(),
    }
}

fn load_openrouter_catalog(provider_dir: &Path) -> Option<ProviderCatalogFile> {
    match load_provider_catalog(provider_dir, Provider::OpenRouter) {
        Ok(Some(catalog)) => Some(catalog),
        _ => None,
    }
}

fn config_from_catalog(catalog: &ProviderCatalogFile, model_id: &ModelId) -> ResolvedModelConfig {
    let defaults = fallback_config();
    let Some(model) = catalog.models.iter().find(|m| m.id == *model_id) else {
        return defaults;
    };
    ResolvedModelConfig {
        compaction_target: resolve_target(model.compaction_target, defaults.compaction_target),
        strip_fraction: resolve_fraction(model.tool_compaction_ratio, defaults.strip_fraction),
        max_iterations: resolve_iterations(model.max_tool_iterations, defaults.max_iterations),
        auto_compact_threshold: resolve_target(
            model.auto_compact_threshold,
            defaults.auto_compact_threshold,
        ),
        max_context_length: model.max_context_length,
    }
}

fn resolve_target(value: TokenCount, fallback: TokenCount) -> TokenCount {
    if value > TokenCount::ZERO {
        value
    } else {
        fallback
    }
}

fn resolve_fraction(
    value: ToolResultStripFraction,
    fallback: ToolResultStripFraction,
) -> ToolResultStripFraction {
    if value > ToolResultStripFraction::ZERO {
        value
    } else {
        fallback
    }
}

fn resolve_iterations(value: Count, fallback: Count) -> Count {
    if value > Count::ZERO {
        value
    } else {
        fallback
    }
}

/// Fallback strip fraction when model config is absent or set to zero (90%).
fn default_strip_fraction() -> ToolResultStripFraction {
    ToolResultStripFraction::new(0.9)
}

fn fallback_config() -> ResolvedModelConfig {
    ResolvedModelConfig {
        compaction_target: FALLBACK_COMPACTION_TARGET,
        strip_fraction: default_strip_fraction(),
        max_iterations: FALLBACK_MAX_ITERATIONS,
        auto_compact_threshold: FALLBACK_AUTO_COMPACT_THRESHOLD,
        max_context_length: TokenCount::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::config::provider_catalog::ProviderCatalogModel;
    use augur_domain::newtypes::CostPerMtok;
    use augur_domain::string_newtypes::{ModelLabel, ProviderName};
    use augur_domain::StringNewtype;

    fn make_catalog_with_model(
        id: &str,
        compaction_target: TokenCount,
        tool_compaction_ratio: ToolResultStripFraction,
        max_tool_iterations: Count,
        auto_compact_threshold: TokenCount,
    ) -> ProviderCatalogFile {
        ProviderCatalogFile {
            provider: ProviderName::new("openrouter"),
            models: vec![ProviderCatalogModel {
                id: ModelId::new(id),
                display_name: Some(ModelLabel::new(id)),
                cost_input_per_mtok: CostPerMtok::ZERO,
                cost_output_per_mtok: CostPerMtok::ZERO,
                supports_tools: Some(true),
                max_context_length: TokenCount::ZERO,
                compaction_target,
                auto_compact_threshold,
                tool_compaction_ratio,
                max_tool_iterations,
            }],
            openrouter: None,
        }
    }

    #[test]
    fn config_from_catalog_uses_model_values() {
        let catalog = make_catalog_with_model(
            "test-model",
            TokenCount::of(200_000),
            ToolResultStripFraction::new(0.5),
            Count::of(50),
            TokenCount::of(150_000),
        );
        let config = config_from_catalog(&catalog, &ModelId::new("test-model"));
        assert_eq!(config.compaction_target, TokenCount::of(200_000));
        assert_eq!(config.strip_fraction, ToolResultStripFraction::new(0.5));
        assert_eq!(config.max_iterations, Count::of(50));
        assert_eq!(config.auto_compact_threshold, TokenCount::of(150_000));
    }

    #[test]
    fn config_from_catalog_zero_fields_fall_back() {
        let catalog = make_catalog_with_model(
            "zero-model",
            TokenCount::ZERO,
            ToolResultStripFraction::ZERO,
            Count::ZERO,
            TokenCount::ZERO,
        );
        let config = config_from_catalog(&catalog, &ModelId::new("zero-model"));
        assert_eq!(config.compaction_target, FALLBACK_COMPACTION_TARGET);
        assert_eq!(config.strip_fraction, super::default_strip_fraction());
        assert_eq!(config.max_iterations, FALLBACK_MAX_ITERATIONS);
        assert_eq!(
            config.auto_compact_threshold,
            FALLBACK_AUTO_COMPACT_THRESHOLD
        );
    }

    #[test]
    fn config_from_catalog_missing_model_falls_back() {
        let catalog = make_catalog_with_model(
            "other-model",
            TokenCount::of(200_000),
            ToolResultStripFraction::new(0.5),
            Count::of(50),
            TokenCount::of(150_000),
        );
        let config = config_from_catalog(&catalog, &ModelId::new("unknown-model"));
        assert_eq!(config.compaction_target, FALLBACK_COMPACTION_TARGET);
        assert_eq!(config.strip_fraction, super::default_strip_fraction());
        assert_eq!(config.max_iterations, FALLBACK_MAX_ITERATIONS);
        assert_eq!(
            config.auto_compact_threshold,
            FALLBACK_AUTO_COMPACT_THRESHOLD
        );
    }

    #[test]
    fn resolve_none_returns_defaults() {
        let config = resolve_model_config(None);
        assert_eq!(config.compaction_target, FALLBACK_COMPACTION_TARGET);
        assert_eq!(config.strip_fraction, super::default_strip_fraction());
        assert_eq!(config.max_iterations, FALLBACK_MAX_ITERATIONS);
        assert_eq!(
            config.auto_compact_threshold,
            FALLBACK_AUTO_COMPACT_THRESHOLD
        );
    }
}
