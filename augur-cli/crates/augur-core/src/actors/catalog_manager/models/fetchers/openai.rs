//! Fetches the model list from the OpenAI API.
//!
//! Endpoint: `GET https://api.openai.com/v1/models`

use anyhow::Result;
use serde::Deserialize;

use super::super::{
    ApiKey, ContextWindowSize, ModelId, ModelInfo, ModelName, ModelPricing, ProviderName,
};
use augur_domain::domain::UsdCost;

const OPENAI_MODELS_URL: &str = "https://api.openai.com/v1/models";

// ── Response shape ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModel {
    id: String,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Fetches available models from the OpenAI API.
///
/// # Arguments
/// - `api_key` - Optional API key used as a Bearer token. Pass `None` to
///   attempt an unauthenticated request (which will likely be rejected by
///   OpenAI in production, but is permitted here for testing).
///
/// # Returns
/// A [`Vec<ModelInfo>`] with one entry per model. Pricing is not published
/// by the `/v1/models` endpoint; all prices are recorded as `0.0` and should
/// be filled from a secondary pricing source if needed.
///
/// # Errors
/// Returns an error if the HTTP request fails or the response cannot be parsed.
pub async fn fetch_models(api_key: Option<ApiKey>) -> Result<Vec<ModelInfo>> {
    fetch_models_from(api_key, OPENAI_MODELS_URL).await
}

// ── Internal (testable) implementation ──────────────────────────────────────

async fn fetch_models_from(api_key: Option<ApiKey>, url: &str) -> Result<Vec<ModelInfo>> {
    let client = reqwest::Client::new();
    let mut request = client.get(url);
    if let Some(key) = api_key {
        request = request.bearer_auth(key.0);
    }
    let response = request.send().await?.json::<OpenAiResponse>().await?;

    let models = response
        .data
        .into_iter()
        .map(|m| ModelInfo {
            name: ModelName::from(m.id.clone()),
            id: ModelId(m.id),
            provider: ProviderName("openai".to_string()),
            context_window: ContextWindowSize(0),
            pricing: ModelPricing {
                input_price_per_mtok: UsdCost::from(0.0),
                output_price_per_mtok: UsdCost::from(0.0),
            },
        })
        .collect();

    Ok(models)
}

// ── Tests ────────────────────────────────────────────────────────────────────
