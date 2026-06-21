//! Provider model-catalog YAML schema and filesystem loader/writer.

use crate::config::types::Provider;
use crate::domain::newtypes::{CostPerMtok, IsEnabled};
use crate::domain::string_newtypes::{ModelId, ModelLabel, ProviderName};
use crate::domain::{Count, TokenCount, ToolResultStripFraction};
use anyhow::Context;
use std::path::{Path, PathBuf};

pub const DEFAULT_PROVIDER_CATALOG_DIR: &str = "configs/providers";

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct OpenRouterCacheConfig {
    #[serde(default)]
    pub enabled: IsEnabled,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u32>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct OpenRouterProviderConfig {
    #[serde(default)]
    pub background_instruction_files: Vec<String>,
    #[serde(default)]
    pub instruction_files: Vec<String>,
    #[serde(default)]
    pub agent_instruction_files: std::collections::HashMap<String, Vec<String>>,
    #[serde(default)]
    pub cache: OpenRouterCacheConfig,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProviderCatalogFile {
    pub provider: ProviderName,
    #[serde(default)]
    pub models: Vec<ProviderCatalogModel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<OpenRouterProviderConfig>,
}

/// Per-model configuration values sourced from the provider YAML catalog.
///
/// Every field uses a zero sentinel to mean "use the provider's default".
/// The resolution logic in `augur_provider_openrouter::model_config`
/// replaces zero values with hardcoded fallbacks.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCatalogModel {
    pub id: ModelId,
    #[serde(default)]
    pub display_name: Option<ModelLabel>,
    pub cost_input_per_mtok: CostPerMtok,
    pub cost_output_per_mtok: CostPerMtok,
    #[serde(default)]
    pub supports_tools: Option<bool>,
    /// Maximum context length in tokens for this model (absolute max the model accepts).
    /// Reserved for future use. 0 means use the provider's default.
    #[serde(default)]
    pub max_context_length: TokenCount,
    /// Target token count after compaction.
    /// When compaction runs, it trims messages down to this target.
    /// 0 means use the provider's default.
    #[serde(default)]
    pub compaction_target: TokenCount,
    /// Token threshold that triggers automatic compaction.
    /// When the estimated request tokens exceed this value, compaction is
    /// triggered toward `compaction_target`.
    /// 0 means use the provider's default (typically 80% of compaction_target).
    #[serde(default)]
    pub auto_compact_threshold: TokenCount,
    /// Fraction of oldest tool-result messages to strip during compaction (0.0-1.0).
    /// 0.0 means use the provider's default.
    #[serde(alias = "compaction_threshold")]
    #[serde(default)]
    pub tool_compaction_ratio: ToolResultStripFraction,
    /// Maximum tool-call iterations before the task stops with a failure.
    /// 0 means use the provider's default.
    #[serde(default)]
    pub max_tool_iterations: Count,
}

pub fn default_provider_catalog_dir() -> PathBuf {
    if let Ok(path) = std::env::var("AUGUR_CLI_PROVIDER_CATALOG_DIR") {
        return PathBuf::from(path);
    }
    let cwd_relative = PathBuf::from(DEFAULT_PROVIDER_CATALOG_DIR);
    if cwd_relative.exists() {
        return cwd_relative;
    }
    // Fall back to installed config directory
    if let Ok(home) = std::env::var("HOME") {
        let install_path = PathBuf::from(home).join(".augur-cli/configs/providers");
        if install_path.exists() {
            return install_path;
        }
    }
    cwd_relative
}

pub fn provider_catalog_path(provider_dir: &Path, provider: Provider) -> PathBuf {
    provider_catalog_path_for_key(provider_dir, provider.to_string().as_str())
}

fn provider_catalog_path_for_key(provider_dir: &Path, provider: &str) -> PathBuf {
    provider_dir.join(format!("{}.yaml", provider.to_lowercase()))
}

pub fn load_provider_catalog(
    provider_dir: &Path,
    provider: Provider,
) -> anyhow::Result<Option<ProviderCatalogFile>> {
    let normalized = provider.to_string().to_lowercase();
    let path = provider_catalog_path_for_key(provider_dir, normalized.as_str());
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading provider catalog file: {}", path.display()))?;
    let parsed: ProviderCatalogFile = serde_yaml::from_str(&raw)
        .with_context(|| format!("parsing provider catalog file: {}", path.display()))?;
    if parsed.provider.to_lowercase() != normalized {
        anyhow::bail!(
            "provider catalog file '{}' declares provider '{}' but expected '{}'",
            path.display(),
            parsed.provider,
            normalized
        );
    }
    Ok(Some(parsed))
}

pub fn write_provider_catalog(
    provider_dir: &Path,
    file: &ProviderCatalogFile,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(provider_dir).with_context(|| {
        format!(
            "creating provider catalog directory: {}",
            provider_dir.display()
        )
    })?;
    let path = provider_catalog_path_for_key(provider_dir, &file.provider);
    let yaml = serde_yaml::to_string(file)
        .with_context(|| format!("serializing provider catalog for '{}'", file.provider))?;
    std::fs::write(&path, yaml)
        .with_context(|| format!("writing provider catalog file: {}", path.display()))?;
    Ok(path)
}
