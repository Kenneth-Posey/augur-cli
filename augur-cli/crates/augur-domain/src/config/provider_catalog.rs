//! Provider model-catalog YAML schema and filesystem loader/writer.

use crate::config::types::Provider;
use crate::domain::newtypes::{CostPerMtok, IsEnabled};
use crate::domain::string_newtypes::{ModelId, ModelLabel, ProviderName, StringNewtype};
use crate::domain::{Count, TokenCount, ToolResultStripFraction};
use crate::tools::definition::ToolDefinition;
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

/// Configuration for a single OpenRouter-provided tool (e.g., web fetch).
///
/// Tools declared here are advertised to the OpenRouter API so that it may
/// offer them to the model without the application implementing a local tool
/// handler for each one.  The `r#type` field follows OpenRouter's tool-type
/// convention (e.g. `"openrouter:web_fetch"`), and the `parameters` block
/// carries tool-specific options.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OpenRouterToolConfig {
    /// Tool type identifier, e.g. `"openrouter:web_fetch"`.
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Tool-specific parameters.
    #[serde(default)]
    pub parameters: OpenRouterToolParameters,
}

/// User location parameters for the `openrouter:web_search` tool.
///
/// Provides an approximate geographic location to the search engine so
/// results can be localized to the user's region.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct UserLocation {
    /// Location type, e.g. `"approximate"`.
    #[serde(rename = "type")]
    pub location_type: String,
    /// City name, e.g. `"San Francisco"`.
    #[serde(default)]
    pub city: String,
    /// Region or state, e.g. `"California"`.
    #[serde(default)]
    pub region: String,
    /// ISO country code, e.g. `"US"`.
    #[serde(default)]
    pub country: String,
    /// IANA timezone, e.g. `"America/Los_Angeles"`.
    #[serde(default)]
    pub timezone: String,
}

/// Parameters for an OpenRouter tool configuration.
///
/// These fields are tool-dependent. The example below shows the parameters
/// for the `openrouter:web_fetch` tool:
///
/// ```yaml
/// parameters:
///   engine: exa
///   max_uses: 10
///   max_content_tokens: 100000
///   allowed_domains:
///     - docs.example.com
///   blocked_domains:
///     - private.example.com
/// ```
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct OpenRouterToolParameters {
    /// Search engine identifier (e.g. `"exa"`), tool-dependent.
    #[serde(default)]
    pub engine: String,
    /// Maximum number of times this tool may be used in a single request.
    #[serde(default)]
    pub max_uses: u64,
    /// Maximum content tokens to return from the tool.
    #[serde(default)]
    pub max_content_tokens: u64,
    /// Only allow results from these domains.
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    /// Block results from these domains.
    #[serde(default)]
    pub blocked_domains: Vec<String>,
    /// Approximate user location for localized search results.
    /// Used by the `openrouter:web_search` tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_location: Option<UserLocation>,
    /// IANA timezone identifier, e.g. `"America/Los_Angeles"`.
    /// Used by the `openrouter:datetime` tool.
    #[serde(default)]
    pub timezone: String,
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
    /// Provider-declared tools that the OpenRouter API may offer to the model.
    ///
    /// These tools do not have local handlers; they are advertised so that
    /// OpenRouter can execute them on the provider side and return results
    /// as tool-call responses.
    #[serde(default)]
    pub tools: Vec<OpenRouterToolConfig>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProviderCatalogFile {
    pub provider: ProviderName,
    #[serde(default)]
    pub models: Vec<ProviderCatalogModel>,
    /// Generic instruction files injected into the main conversation context
    /// for this provider. Shared across all provider types.
    #[serde(default)]
    pub instruction_files: Vec<String>,
    /// Generic instruction files injected into background agent/dispatched-task
    /// contexts for this provider. Shared across all provider types.
    #[serde(default)]
    pub background_instruction_files: Vec<String>,
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
    /// Maximum tool response tokens before the output is replaced with a
    /// warning asking the LLM to use a more targeted call.
    /// 0 means use the provider's default (50_000).
    #[serde(default)]
    pub tool_response_cap: TokenCount,
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
        let install_path = PathBuf::from(home).join(".augur-cli/config/providers");
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

/// Build a human-readable `ToolDescription` for a provider-declared tool type.
fn tool_description_for_type(tool_type: &str) -> &'static str {
    match tool_type {
        "openrouter:web_fetch" => {
            "Fetch the content of a web page or URL. Returns the full text content of the page."
        }
        "openrouter:web_search" => {
            "Search the web for current information. Returns search results relevant to the query."
        }
        "openrouter:datetime" => {
            "Get the current date and time in a specified timezone. Returns the current datetime."
        }
        _ => "Provider-declared tool available through the OpenRouter API.",
    }
}

/// Convert an `OpenRouterToolConfig` into a `ToolDefinition` suitable for
/// inclusion in the LLM tools array.
///
/// The `tool_type` (e.g. `"openrouter:web_fetch"`) becomes the tool name.
/// A human-readable description is derived from the tool type. The
/// `OpenRouterToolParameters` are serialized into a JSON Schema object
/// describing the expected parameters.
pub fn openrouter_tool_to_definition(tool: &OpenRouterToolConfig) -> ToolDefinition {
    use crate::domain::string_newtypes::{ToolDescription, ToolName};

    let name = ToolName::new(tool.tool_type.as_str());
    let description = ToolDescription::new(tool_description_for_type(&tool.tool_type));
    let parameters = build_parameters_schema(&tool.tool_type, &tool.parameters);

    ToolDefinition::new(name, description, parameters)
}

/// Build a JSON Schema `parameters` object for the given tool type and parameters.
fn build_parameters_schema(
    tool_type: &str,
    params: &OpenRouterToolParameters,
) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let required: Vec<String> = Vec::new();

    match tool_type {
        "openrouter:web_fetch" => {
            if !params.engine.is_empty() {
                properties.insert(
                    "engine".into(),
                    serde_json::json!({
                        "type": "string",
                        "description": "Search engine identifier (e.g. \"exa\")"
                    }),
                );
            }
            if params.max_uses > 0 {
                properties.insert(
                    "max_uses".into(),
                    serde_json::json!({
                        "type": "integer",
                        "description": "Maximum number of times this tool may be used in a single request"
                    }),
                );
            }
            if params.max_content_tokens > 0 {
                properties.insert(
                    "max_content_tokens".into(),
                    serde_json::json!({
                        "type": "integer",
                        "description": "Maximum content tokens to return from the tool"
                    }),
                );
            }
            if !params.allowed_domains.is_empty() {
                properties.insert(
                    "allowed_domains".into(),
                    serde_json::json!({
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Only allow results from these domains"
                    }),
                );
            }
            if !params.blocked_domains.is_empty() {
                properties.insert(
                    "blocked_domains".into(),
                    serde_json::json!({
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Block results from these domains"
                    }),
                );
            }
        }
        "openrouter:web_search" => {
            if let Some(ref loc) = params.user_location {
                let mut loc_props = serde_json::Map::new();
                if !loc.location_type.is_empty() {
                    loc_props.insert(
                        "type".into(),
                        serde_json::json!({
                            "type": "string",
                            "description": "Location type (e.g. \"approximate\")"
                        }),
                    );
                }
                if !loc.city.is_empty() {
                    loc_props.insert(
                        "city".into(),
                        serde_json::json!({
                            "type": "string",
                            "description": "City name"
                        }),
                    );
                }
                if !loc.region.is_empty() {
                    loc_props.insert(
                        "region".into(),
                        serde_json::json!({
                            "type": "string",
                            "description": "Region or state"
                        }),
                    );
                }
                if !loc.country.is_empty() {
                    loc_props.insert(
                        "country".into(),
                        serde_json::json!({
                            "type": "string",
                            "description": "ISO country code"
                        }),
                    );
                }
                if !loc.timezone.is_empty() {
                    loc_props.insert(
                        "timezone".into(),
                        serde_json::json!({
                            "type": "string",
                            "description": "IANA timezone"
                        }),
                    );
                }
                properties.insert(
                    "user_location".into(),
                    serde_json::json!({
                        "type": "object",
                        "properties": loc_props,
                        "description": "Approximate user location for localized search results"
                    }),
                );
            }
        }
        "openrouter:datetime" => {
            if !params.timezone.is_empty() {
                properties.insert(
                    "timezone".into(),
                    serde_json::json!({
                        "type": "string",
                        "description": "IANA timezone identifier (e.g. \"America/Los_Angeles\")"
                    }),
                );
            }
        }
        _ => {}
    }

    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
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
