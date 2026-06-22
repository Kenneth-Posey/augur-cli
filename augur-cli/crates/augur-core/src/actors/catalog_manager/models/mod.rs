//! Model catalog shared types.
//!
//! Defines [`ModelInfo`], [`ModelPricing`], and [`FilterOpts`] used across
//! the fetchers, filter, and formatter modules.

use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::ModelName;
use augur_domain::domain::UsdCost;
use serde::{Deserialize, Serialize};

/// An opaque provider authentication key.
/// Prevents accidental confusion with arbitrary string values at call sites.
#[derive(Debug, Clone)]
pub struct ApiKey(pub String);

/// YAML snippet ready to paste into `application.yaml`.
pub struct YamlSnippet(pub String);

/// GitHub-Flavoured Markdown model catalog table.
pub struct MarkdownCatalog(pub String);

/// Output format for the catalog.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Yaml,
    Markdown,
}

/// Which provider(s) to fetch models from.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ProviderChoice {
    Openai,
    Anthropic,
    Openrouter,
    Ollama,
    All,
}

/// Unique model identifier (e.g. `"gpt-4o"`, `"claude-3-5-sonnet-20241022"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(pub String);

/// Provider name (e.g. `"openai"`, `"anthropic"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderName(pub String);

/// Token count for a context window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextWindowSize(pub u32);

pub mod fetchers;
pub mod filter;
pub mod formatter;

/// Pricing per million tokens for a model.
///
/// Both prices are in USD. The `_per_mtok` suffix indicates per-million-token
/// units, consistent with how providers publish list prices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Cost in USD per million input (prompt) tokens.
    pub input_price_per_mtok: UsdCost,
    /// Cost in USD per million output (completion) tokens.
    pub output_price_per_mtok: UsdCost,
}

/// Metadata for a single language model returned by a provider API.
///
/// The struct is kept to exactly five fields; additional capability flags
/// (e.g., tool-use support) are inferred by the filter layer from provider
/// and id conventions rather than stored here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Canonical model identifier used in API calls (e.g., `"gpt-4-turbo"`).
    pub id: ModelId,
    /// Human-readable display name returned by the provider.
    pub name: ModelName,
    /// Provider name in lowercase (e.g., `"openai"`, `"anthropic"`, `"ollama"`).
    pub provider: ProviderName,
    /// Maximum context window in tokens reported by the provider.
    pub context_window: ContextWindowSize,
    /// Per-million-token pricing for input and output.
    pub pricing: ModelPricing,
}

/// Cost ceiling tier applied to input price per million tokens.
#[derive(Debug, Clone, PartialEq)]
pub enum CostTier {
    /// ≤ $1.00/Mtok input price
    Budget,
    /// ≤ $5.00/Mtok input price
    Standard,
    /// ≤ $20.00/Mtok input price
    Premium,
}

/// CLI filter parameters that control which models are emitted.
///
/// Build with [`FilterOpts::builder()`]. Boolean fields default to `false`
/// and `Option` fields default to `None` when not supplied.
#[derive(Debug, Clone, bon::Builder)]
pub struct FilterOpts {
    /// When `Some(name)`, restrict output to models from that provider.
    pub provider_filter: Option<ProviderName>,
    /// When `true`, omit models from providers that do not support tool use.
    #[builder(default = IsPredicate::no())]
    pub tool_use_only: IsPredicate,
    /// When `true`, keep only the lexicographically latest model id per
    /// `(provider, family)` group, where family strips trailing date/version
    /// suffixes such as `-20240229` or `-0613`.
    #[builder(default = IsPredicate::no())]
    pub latest_only: IsPredicate,
    /// Optional cost-tier ceiling applied to input price per million tokens.
    ///
    /// Use [`CostTier::Budget`] (≤ $1.00), [`CostTier::Standard`] (≤ $5.00),
    /// or [`CostTier::Premium`] (≤ $20.00). `None` passes all models through.
    pub max_cost_tier: Option<CostTier>,
}
