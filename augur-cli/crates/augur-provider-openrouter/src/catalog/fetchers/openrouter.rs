//! Fetches the model list from the OpenRouter API.
//!
//! Endpoint: `GET https://openrouter.ai/api/v1/models`

use anyhow::Result;
use serde::Deserialize;

use super::super::{ApiKey, ContextWindowSize, ModelId, ModelInfo, ModelPricing, ProviderName};
use augur_domain::UsdCost;

const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";

/// Conversion factor: OpenRouter prices are per-token; multiply by this to
/// get per-million-token values.
const TOKENS_PER_MTOK: f64 = 1_000_000.0;

// ── Response shape ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    name: Option<String>,
    pricing: Option<OpenRouterPricing>,
    context_length: Option<u32>,
}

/// OpenRouter returns prices as decimal strings in USD **per token**.
#[derive(Debug, Deserialize)]
struct OpenRouterPricing {
    prompt: Option<String>,
    completion: Option<String>,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Fetches available models from the OpenRouter model catalogue.
///
/// # Arguments
/// - `api_key` - Optional API key for authenticated requests. Pass `None` to
///   retrieve the public model list without authentication.
///
/// # Returns
/// A [`Vec<ModelInfo>`] with one entry per model returned by the API.
///
/// # Errors
/// Returns an error if the HTTP request fails or the response body cannot
/// be deserialised.
pub async fn fetch_models(api_key: Option<ApiKey>) -> Result<Vec<ModelInfo>> {
    fetch_models_from(api_key, OPENROUTER_MODELS_URL).await
}

// ── Internal (testable) implementation ──────────────────────────────────────

async fn fetch_models_from(api_key: Option<ApiKey>, url: &str) -> Result<Vec<ModelInfo>> {
    let client = reqwest::Client::new();
    let mut request = client.get(url);
    if let Some(key) = api_key {
        request = request.bearer_auth(key.0);
    }
    let response = request.send().await?.json::<OpenRouterResponse>().await?;

    let models = response
        .data
        .into_iter()
        .map(|m| {
            let (input_price, output_price) = parse_openrouter_prices(m.pricing.as_ref());
            ModelInfo {
                id: ModelId(m.id),
                name: m.name.unwrap_or_default(),
                provider: ProviderName("openrouter".to_string()),
                context_window: ContextWindowSize(m.context_length.unwrap_or(0)),
                pricing: ModelPricing {
                    input_price_per_mtok: UsdCost::from(input_price),
                    output_price_per_mtok: UsdCost::from(output_price),
                },
            }
        })
        .collect();

    Ok(models)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Parses OpenRouter per-token string prices into per-million-token `f64` values.
///
/// OpenRouter encodes prices as decimal strings (e.g., `"0.000015"`).
/// Multiplying by `1_000_000` converts to the per-million-token convention.
fn parse_openrouter_prices(pricing: Option<&OpenRouterPricing>) -> (f64, f64) {
    let Some(p) = pricing else {
        return (0.0, 0.0);
    };
    let input = p
        .prompt
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
        * TOKENS_PER_MTOK;
    let output = p
        .completion
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
        * TOKENS_PER_MTOK;
    (input, output)
}

// ── Tests ────────────────────────────────────────────────────────────────────
