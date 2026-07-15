//! Per-model configuration resolution from provider catalog YAML files.
//!
//! Loads the OpenRouter provider catalog at runtime and extracts per-model
//! values for compaction target, strip fraction, max tool iterations, and
//! auto-compact threshold.
//! Every value falls back to a hardcoded default when the model is absent or
//! the field is set to its zero sentinel (meaning "use provider default").

use augur_domain::config::provider_catalog::{
    ProviderCatalogFile, default_provider_catalog_dir, load_provider_catalog,
};
use augur_domain::config::types::Provider;
use augur_domain::newtypes::{Count, NumericNewtype, TokenCount, ToolResultStripFraction};
use augur_domain::string_newtypes::ModelId;
use std::path::Path;

// ── Default values ────────────────────────────────────────────────────────────

/// Default max context length in tokens when no per-model configuration is available.
///
/// Used as a safe fallback when the provider catalog does not specify a
/// `max_context_length` for the current model. Mirrors the identical constant
/// in the main agent (`augur_core::actors::agent::agent_ops::DEFAULT_MAX_CONTEXT_LENGTH`).
const DEFAULT_MAX_CONTEXT_LENGTH: TokenCount = TokenCount::of(200_000);

/// Fraction of `max_context_length` used as the total request-size guard threshold
/// when `auto_compact_threshold` is not set.
///
/// The remaining headroom (20%) accounts for system prompt, tool definitions,
/// and serialization overhead. Mirrors the identical computation in
/// `augur_core::actors::agent::assistant_core`.
const CAP_FRACTION_NUMERATOR: u64 = 80;
const CAP_FRACTION_DENOMINATOR: u64 = 100;

/// Fallback compaction target when model config is absent or set to zero (400k tokens).
const FALLBACK_COMPACTION_TARGET: TokenCount = TokenCount::of(400_000);

/// Fallback max tool iterations when model config is absent or set to zero (100).
const FALLBACK_MAX_ITERATIONS: Count = Count::of(100);

/// Fallback auto-compact threshold when model config is absent or set to zero.
/// Computed as 80% of `FALLBACK_COMPACTION_TARGET` (320_000 tokens).
const FALLBACK_AUTO_COMPACT_THRESHOLD: TokenCount = TokenCount::of(320_000);

/// Fallback tool response cap when model config is absent or set to zero (50_000 tokens).
const FALLBACK_TOOL_RESPONSE_CAP: TokenCount = TokenCount::of(50_000);

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
    /// Maximum tool response tokens before the output is replaced with a
    /// warning asking the LLM to use a more targeted call.
    /// Falls back to [`FALLBACK_TOOL_RESPONSE_CAP`] (50_000) when not set.
    pub tool_response_cap: TokenCount,
}

impl ResolvedModelConfig {
    /// Compute the effective request-size cap, following the same logic as the
    /// main agent in `assistant_core::effective_request_cap`.
    ///
    /// 1. Prefers `auto_compact_threshold` when set (> 0).
    /// 2. Falls back to `max_context_length * 80%` when `max_context_length > 0`.
    /// 3. Falls back to `DEFAULT_MAX_CONTEXT_LENGTH * 80%` (160K) when neither
    ///    is configured.
    pub fn effective_request_cap(&self) -> TokenCount {
        if self.auto_compact_threshold > TokenCount::ZERO {
            self.auto_compact_threshold
        } else if self.max_context_length > TokenCount::ZERO {
            TokenCount::new(
                self.max_context_length.inner() * CAP_FRACTION_NUMERATOR / CAP_FRACTION_DENOMINATOR,
            )
        } else {
            TokenCount::new(
                DEFAULT_MAX_CONTEXT_LENGTH.inner() * CAP_FRACTION_NUMERATOR
                    / CAP_FRACTION_DENOMINATOR,
            )
        }
    }
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
        tool_response_cap: resolve_target(model.tool_response_cap, defaults.tool_response_cap),
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
    if value > Count::ZERO { value } else { fallback }
}

/// Fallback strip fraction when model config is absent or set to zero (90%).
fn default_strip_fraction() -> ToolResultStripFraction {
    ToolResultStripFraction::new(0.9)
}

/// Return a `ResolvedModelConfig` populated with hardcoded fallback defaults.
///
/// Useful for test builders that need a valid config value but don't care
/// about specific thresholds.
pub fn fallback_default_model_config() -> ResolvedModelConfig {
    fallback_config()
}

fn fallback_config() -> ResolvedModelConfig {
    ResolvedModelConfig {
        compaction_target: FALLBACK_COMPACTION_TARGET,
        strip_fraction: default_strip_fraction(),
        max_iterations: FALLBACK_MAX_ITERATIONS,
        auto_compact_threshold: FALLBACK_AUTO_COMPACT_THRESHOLD,
        max_context_length: TokenCount::ZERO,
        tool_response_cap: FALLBACK_TOOL_RESPONSE_CAP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use augur_domain::StringNewtype;
    use augur_domain::config::provider_catalog::ProviderCatalogModel;
    use augur_domain::newtypes::CostPerMtok;
    use augur_domain::string_newtypes::{ModelLabel, ProviderName};

    fn make_catalog_with_model(
        id: &str,
        compaction_target: TokenCount,
        tool_compaction_ratio: ToolResultStripFraction,
        max_tool_iterations: Count,
        auto_compact_threshold: TokenCount,
        tool_response_cap: TokenCount,
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
                tool_response_cap,
            }],
            instruction_files: Vec::new(),
            background_instruction_files: Vec::new(),
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
            TokenCount::of(100_000),
        );
        let config = config_from_catalog(&catalog, &ModelId::new("test-model"));
        assert_eq!(config.compaction_target, TokenCount::of(200_000));
        assert_eq!(config.strip_fraction, ToolResultStripFraction::new(0.5));
        assert_eq!(config.max_iterations, Count::of(50));
        assert_eq!(config.auto_compact_threshold, TokenCount::of(150_000));
        assert_eq!(config.tool_response_cap, TokenCount::of(100_000));
    }

    #[test]
    fn config_from_catalog_zero_fields_fall_back() {
        let catalog = make_catalog_with_model(
            "zero-model",
            TokenCount::ZERO,
            ToolResultStripFraction::ZERO,
            Count::ZERO,
            TokenCount::ZERO,
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
        assert_eq!(config.tool_response_cap, FALLBACK_TOOL_RESPONSE_CAP);
    }

    #[test]
    fn config_from_catalog_missing_model_falls_back() {
        let catalog = make_catalog_with_model(
            "other-model",
            TokenCount::of(200_000),
            ToolResultStripFraction::new(0.5),
            Count::of(50),
            TokenCount::of(150_000),
            TokenCount::of(100_000),
        );
        let config = config_from_catalog(&catalog, &ModelId::new("unknown-model"));
        assert_eq!(config.compaction_target, FALLBACK_COMPACTION_TARGET);
        assert_eq!(config.strip_fraction, super::default_strip_fraction());
        assert_eq!(config.max_iterations, FALLBACK_MAX_ITERATIONS);
        assert_eq!(
            config.auto_compact_threshold,
            FALLBACK_AUTO_COMPACT_THRESHOLD
        );
        assert_eq!(config.tool_response_cap, FALLBACK_TOOL_RESPONSE_CAP);
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
        assert_eq!(config.tool_response_cap, FALLBACK_TOOL_RESPONSE_CAP);
    }
}
