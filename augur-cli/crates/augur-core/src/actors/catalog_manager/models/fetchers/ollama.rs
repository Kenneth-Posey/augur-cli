//! Fetches the local model list from a running Ollama instance.
//!
//! Endpoint: `GET http://localhost:11434/api/tags`
//!
//! No API key is required. The base URL is fixed to the Ollama default
//! (`http://localhost:11434`). If the instance is not running, the fetch
//! returns an error rather than an empty list.

use anyhow::Result;
use serde::Deserialize;

use super::super::{ContextWindowSize, ModelId, ModelInfo, ModelName, ModelPricing, ProviderName};
use augur_domain::domain::UsdCost;

const OLLAMA_TAGS_URL: &str = "http://localhost:11434/api/tags";

// ── Response shape ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Fetches locally available models from the Ollama daemon at
/// `http://localhost:11434`.
///
/// # Returns
/// A [`Vec<ModelInfo>`] with one entry per tag returned by `/api/tags`.
/// Context-window size and pricing are unavailable from Ollama and are
/// recorded as `0` / `0.0`.
///
/// # Errors
/// Returns an error if Ollama is not running or the response cannot be parsed.
pub async fn fetch_models() -> Result<Vec<ModelInfo>> {
    fetch_models_from(OLLAMA_TAGS_URL).await
}

// ── Internal (testable) implementation ──────────────────────────────────────

async fn fetch_models_from(url: &str) -> Result<Vec<ModelInfo>> {
    let response = reqwest::get(url)
        .await?
        .json::<OllamaTagsResponse>()
        .await?;

    let models = response
        .models
        .into_iter()
        .map(|m| ModelInfo {
            name: ModelName::from(m.name.clone()),
            id: ModelId(m.name),
            provider: ProviderName("ollama".to_string()),
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
