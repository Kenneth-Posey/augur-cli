//! Pure filtering logic for [`ModelInfo`] slices.
//!
//! All functions are deterministic and free of I/O side-effects, making them
//! straightforward to unit-test without any HTTP infrastructure.

use super::{CostTier, FilterOpts, ModelInfo};
use augur_domain::domain::newtypes::IsPredicate;

// ── Tier thresholds (input price per million tokens, USD) ───────────────────

const BUDGET_THRESHOLD: f64 = 1.0;
const STANDARD_THRESHOLD: f64 = 5.0;
const PREMIUM_THRESHOLD: f64 = 20.0;

/// Providers known to support tool/function calling.
///
/// Ollama models are excluded because tool-use availability depends on the
/// specific model variant and cannot be inferred from the provider name alone.
const TOOL_USE_PROVIDERS: &[&str] = &["openai", "anthropic", "openrouter"];

// ── Public API ──────────────────────────────────────────────────────────────

/// Filters and optionally deduplicates a list of models according to `opts`.
///
/// Filters are applied in order:
/// 1. **Provider filter** - keep only models whose `provider` matches
///    `opts.provider_filter` (case-insensitive).
/// 2. **Tool-use filter** - when `opts.tool_use_only` is `true`, keep only
///    models from providers known to support tool/function calling (openai,
///    anthropic, openrouter). Ollama is excluded because availability depends
///    on the specific model variant.
/// 3. **Cost-tier filter** - when `opts.max_cost_tier` is `Some`, discard
///    models whose input price exceeds the tier ceiling.
/// 4. **Latest-only deduplication** - when `opts.latest_only` is `true`,
///    retain only the lexicographically latest model id per
///    `(provider, family)` group where *family* is the id with trailing
///    date/version suffixes (e.g., `-20240229`, `-0613`) stripped.
///
/// The resulting slice preserves the relative order of surviving models.
///
/// # Arguments
/// - `models` - Owned list to filter; consumed and rebuilt to avoid cloning.
/// - `opts`   - Reference to filter parameters built with [`FilterOpts::builder()`].
///
/// # Returns
/// A new `Vec<ModelInfo>` containing only the models that passed all filters.
pub fn filter_models(models: Vec<ModelInfo>, opts: &FilterOpts) -> Vec<ModelInfo> {
    let after_provider =
        apply_provider_filter(models, opts.provider_filter.as_ref().map(|p| p.0.as_str()));
    let after_tool_use = apply_tool_use_filter(after_provider, opts.tool_use_only);
    let after_cost = apply_cost_tier_filter(after_tool_use, opts.max_cost_tier.as_ref());
    apply_latest_only(after_cost, opts.latest_only)
}

// ── Filter steps ────────────────────────────────────────────────────────────

/// Keeps only models whose `provider` equals `filter` (case-insensitive).
///
/// When `filter` is `None`, all models are passed through unchanged.
fn apply_provider_filter(models: Vec<ModelInfo>, filter: Option<&str>) -> Vec<ModelInfo> {
    let Some(name) = filter else {
        return models;
    };
    let name_lower = name.to_lowercase();
    models
        .into_iter()
        .filter(|m| m.provider.0.to_lowercase() == name_lower)
        .collect()
}

/// Removes models from providers that do not support tool/function calling.
///
/// When `enabled` is `false`, all models are returned unchanged.
fn apply_tool_use_filter(models: Vec<ModelInfo>, enabled: IsPredicate) -> Vec<ModelInfo> {
    if !enabled.0 {
        return models;
    }
    models
        .into_iter()
        .filter(|m| TOOL_USE_PROVIDERS.contains(&m.provider.0.as_str()))
        .collect()
}

/// Removes models whose input price per million tokens exceeds the tier ceiling.
///
/// | Tier       | Max input $/Mtok |
/// |------------|-----------------|
/// | `Budget`   | 1.0             |
/// | `Standard` | 5.0             |
/// | `Premium`  | 20.0            |
///
/// When `tier` is `None`, all models are returned unchanged.
fn apply_cost_tier_filter(models: Vec<ModelInfo>, tier: Option<&CostTier>) -> Vec<ModelInfo> {
    let Some(ceiling) = tier_ceiling(tier) else {
        return models;
    };
    models
        .into_iter()
        .filter(|m| *m.pricing.input_price_per_mtok <= ceiling)
        .collect()
}

/// For each `(provider, family)` group, retains the model with the
/// lexicographically largest `id` (a proxy for recency).
///
/// When `enabled` is `false`, all models are returned unchanged.
fn apply_latest_only(models: Vec<ModelInfo>, enabled: IsPredicate) -> Vec<ModelInfo> {
    if !enabled.0 {
        return models;
    }

    // Build a map: (provider, family) → best model seen so far.
    let mut best: std::collections::HashMap<(String, String), ModelInfo> =
        std::collections::HashMap::new();

    for model in models {
        let family = model_family(&model.id.0).to_string();
        let key = (model.provider.0.clone(), family);
        let is_better = best
            .get(&key)
            .is_none_or(|existing| model.id.0 > existing.id.0);
        if is_better {
            best.insert(key, model);
        }
    }

    // Collect and sort deterministically (provider asc, id asc).
    let mut result: Vec<ModelInfo> = best.into_values().collect();
    result.sort_by(|a, b| {
        a.provider
            .0
            .cmp(&b.provider.0)
            .then_with(|| a.id.0.cmp(&b.id.0))
    });
    result
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Returns the cost-tier ceiling in USD/Mtok, or `None` when `tier` is `None`.
///
/// # Arguments
/// - `tier` - Optional [`CostTier`] variant.
///
/// # Returns
/// `Some(f64)` ceiling or `None` if `tier` is `None`.
fn tier_ceiling(tier: Option<&CostTier>) -> Option<f64> {
    match tier? {
        CostTier::Budget => Some(BUDGET_THRESHOLD),
        CostTier::Standard => Some(STANDARD_THRESHOLD),
        CostTier::Premium => Some(PREMIUM_THRESHOLD),
    }
}

/// Strips a trailing date or numeric version suffix from a model id.
///
/// The suffix must be a hyphen followed by four or more consecutive ASCII
/// digits (e.g., `-20240229`, `-0613`, `-1106`). The first such suffix from
/// the right is removed.
///
/// # Examples
///
/// * `"gpt-4-0613"` → `"gpt-4"`
/// * `"claude-3-5-sonnet-20241022"` → `"claude-3-5-sonnet"`
fn model_family(id: &str) -> &str {
    if let Some(pos) = id.rfind('-') {
        let suffix = &id[pos + 1..];
        let is_version = suffix.len() >= 4 && suffix.chars().all(|c| c.is_ascii_digit());
        if is_version {
            return &id[..pos];
        }
    }
    id
}

// ── Tests ────────────────────────────────────────────────────────────────────
