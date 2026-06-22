//! Fetches the model list from the Anthropic API.
//!
//! Endpoint: `GET https://api.anthropic.com/v1/models`

use anyhow::Result;
use serde::Deserialize;

use super::super::{
    ApiKey, ContextWindowSize, ModelId, ModelInfo, ModelName, ModelPricing, ProviderName,
};
use augur_domain::domain::UsdCost;

const ANTHROPIC_MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const ANTHROPIC_API_VERSION: &str = "2023-06-01";

// ── Response shape ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
    display_name: Option<String>,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Fetches available models from the Anthropic API.
///
/// # Arguments
/// - `api_key` - Optional API key sent in the `x-api-key` header. Pass `None`
///   to attempt the request without authentication.
///
/// # Returns
/// A [`Vec<ModelInfo>`] with one entry per model. Pricing is not returned by
/// the models endpoint; all prices are recorded as `0.0`.
///
/// # Errors
/// Returns an error if the HTTP request fails or the response body cannot be
/// deserialised.
pub async fn fetch_models(api_key: Option<ApiKey>) -> Result<Vec<ModelInfo>> {
    fetch_models_from(api_key, ANTHROPIC_MODELS_URL).await
}

// ── Internal (testable) implementation ──────────────────────────────────────

async fn fetch_models_from(api_key: Option<ApiKey>, url: &str) -> Result<Vec<ModelInfo>> {
    let client = reqwest::Client::new();
    let mut request = client
        .get(url)
        .header("anthropic-version", ANTHROPIC_API_VERSION);
    if let Some(key) = api_key {
        request = request.header("x-api-key", key.0);
    }
    let response = request.send().await?.json::<AnthropicResponse>().await?;

    let models = response
        .data
        .into_iter()
        .map(|m| {
            let name = m.display_name.unwrap_or_else(|| m.id.clone());
            ModelInfo {
                id: ModelId(m.id),
                name: ModelName::from(name),
                provider: ProviderName("anthropic".to_string()),
                context_window: ContextWindowSize(0),
                pricing: ModelPricing {
                    input_price_per_mtok: UsdCost::from(0.0),
                    output_price_per_mtok: UsdCost::from(0.0),
                },
            }
        })
        .collect();

    Ok(models)
}

// ── Tests ────────────────────────────────────────────────────────────────────
