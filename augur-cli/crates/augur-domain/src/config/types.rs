//! Configuration types: endpoint, agent, and application-level config.

use crate::domain::newtypes::Temperature;
use crate::domain::string_newtypes::{ApiKey, BearerToken, EnvVarName, FilePath, StringNewtype};
use crate::domain::{EndpointName, EndpointUrl, IsPredicate, ModelName, OutputText, TokenCount};

/// Identifies the LLM API provider for an endpoint.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Provider {
    OpenAi,
    Anthropic,
    Ollama,
    /// OpenRouter API gateway - routes to many upstream models via a single endpoint.
    ///
    /// Set `provider: OpenRouter` in `application.yaml` and supply
    /// `OPENROUTER_API_KEY` (or override via `api_key_env`) to use this provider.
    OpenRouter,
}

impl std::fmt::Display for Provider {
    /// Format as a short lowercase provider label (e.g. `"openai"`, `"ollama"`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Provider::OpenAi => "openai",
            Provider::Anthropic => "anthropic",
            Provider::Ollama => "ollama",
            Provider::OpenRouter => "openrouter",
        };
        f.write_str(label)
    }
}

/// Credential sources for a single LLM endpoint.
///
/// Uses semantic wrappers so environment-variable names and direct API-key values
/// are never exposed as bare strings in the public configuration API.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EndpointCredentials {
    /// Environment variable name holding the API key.
    ///
    /// Uses [`EnvVarName`]. `None` means the endpoint does not require an
    /// environment variable for authentication.
    pub api_key_env: Option<EnvVarName>,
    /// Direct API key value, typically supplied via `application.secrets.yaml`.
    ///
    /// Uses [`ApiKey`]. When set, this takes precedence over `api_key_env`.
    #[serde(default)]
    pub api_key: Option<ApiKey>,
}

/// Configuration for a single named LLM endpoint.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EndpointConfig {
    /// Unique human-readable key used to select this endpoint by name.
    pub name: EndpointName,
    /// Which API provider handles requests to this endpoint.
    pub provider: Provider,
    /// Base URL for the provider's API (no trailing slash needed).
    pub base_url: EndpointUrl,
    /// Model identifier sent in each API request.
    pub model: ModelName,
    /// Credential sources expressed via [`EndpointCredentials`], [`EnvVarName`],
    /// and [`ApiKey`] wrappers instead of bare strings.
    #[serde(flatten)]
    pub credentials: EndpointCredentials,
}

/// Behavioral configuration for the agent conversation loop.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentConfig {
    /// Initial system prompt prepended to every conversation.
    pub system_prompt: OutputText,
    /// Maximum tokens the LLM may generate per response.
    pub max_tokens: TokenCount,
    /// Sampling temperature forwarded to the LLM API.
    pub temperature: Temperature,
    /// Directories the file-read tools are permitted to access.
    ///
    /// Relative paths are resolved from the current working directory at startup.
    /// Defaults to `["./"]` (the current working directory only) when absent from config.
    #[serde(default = "default_allowed_dirs")]
    pub allowed_dirs: Vec<FilePath>,
}

fn default_allowed_dirs() -> Vec<FilePath> {
    vec![FilePath::new("./")]
}

fn default_log_dir() -> FilePath {
    FilePath::new("./logs")
}

/// Project-owned runtime settings that can be adjusted without code changes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProgramSettings {
    /// Directory names excluded from directory listings.
    #[serde(default = "default_excluded_directories")]
    pub excluded_directories: Vec<FilePath>,
}

impl Default for ProgramSettings {
    fn default() -> Self {
        Self {
            excluded_directories: default_excluded_directories(),
        }
    }
}

fn default_excluded_directories() -> Vec<FilePath> {
    vec![
        FilePath::new(".git"),
        FilePath::new("target"),
        FilePath::new("changelogs"),
    ]
}

impl ProgramSettings {
    /// Return the excluded directories as owned filesystem paths.
    pub fn excluded_directory_paths(&self) -> Vec<std::path::PathBuf> {
        self.excluded_directories
            .iter()
            .map(|p| std::path::PathBuf::from(p.as_str()))
            .collect()
    }
}

/// Lightweight user preferences persisted across sessions.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserSettings {
    /// Last active endpoint name (e.g. "openrouter", "copilot").
    #[serde(default)]
    pub last_endpoint: Option<String>,
    /// Last active model ID. `None` means endpoint default.
    #[serde(default)]
    pub last_model: Option<String>,
    /// Last selected reasoning effort level. `None` means unset.
    #[serde(default)]
    pub last_reasoning_effort: Option<String>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            last_endpoint: Some("openrouter".to_owned()),
            last_model: Some("deepseek/deepseek-v4-flash".to_owned()),
            last_reasoning_effort: Some("high".to_owned()),
        }
    }
}

/// Filesystem-backed persistence paths used by the application.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistenceConfig {
    /// Directory where per-session JSONL log files are written.
    ///
    /// Uses [`FilePath`] instead of a bare string. Relative paths are resolved
    /// from the working directory when the application starts. Defaults to
    /// `"./logs"` when omitted from the config file.
    #[serde(default = "default_log_dir")]
    pub log_dir: FilePath,
    /// Directory where session JSON files are stored.
    ///
    /// Uses [`FilePath`] instead of a bare string. Supports `~` as a prefix for
    /// the user's home directory. When `None`, defaults to
    /// `~/.augur-cli/sessions`. Panics at startup if `HOME` is not set.
    #[serde(default)]
    pub sessions_dir: Option<FilePath>,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            log_dir: default_log_dir(),
            sessions_dir: None,
        }
    }
}

/// Shared GitHub Copilot SDK connection settings.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CopilotSdkSettings {
    /// Path to the GitHub Copilot CLI binary.
    ///
    /// Uses [`FilePath`] instead of a bare string. When `None`, the runtime
    /// locates the CLI on `$PATH`.
    pub cli_path: Option<FilePath>,
    /// Model identifier passed to the SDK session.
    ///
    /// Uses [`ModelName`] instead of a bare string. When `None`, the SDK uses
    /// its session default.
    pub model: Option<ModelName>,
    /// Explicit bearer token for GitHub authentication.
    ///
    /// Uses [`BearerToken`] instead of a bare string. When `None`, the runtime
    /// falls back to ambient CLI or environment-based credentials.
    pub auth_token: Option<BearerToken>,
    /// Whether to use the currently logged-in `gh` CLI user.
    pub use_logged_in_user: Option<IsPredicate>,
}

/// Configuration for the GitHub Copilot chat actor.
///
/// Active when `enabled: true` and the `copilot-executor` feature is compiled in.
/// Loaded from the `copilot_chat:` section in `application.yaml`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CopilotChatConfig {
    /// When `true`, the SDK chat actor is the primary chat backend.
    #[serde(default)]
    pub enabled: IsPredicate,
    /// Shared SDK settings using [`CopilotSdkSettings`], [`FilePath`],
    /// [`ModelName`], and [`BearerToken`] wrappers.
    #[serde(flatten)]
    pub sdk: CopilotSdkSettings,
}

/// Configuration for the optional GitHub Copilot CLI executor.
///
/// Active only when the `copilot-executor` feature is enabled.
/// Loaded from the `executor:` section in `application.yaml`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ExecutorConfig {
    /// Shared SDK settings using [`CopilotSdkSettings`], [`FilePath`],
    /// [`ModelName`], and [`BearerToken`] wrappers.
    #[serde(flatten)]
    pub sdk: CopilotSdkSettings,
}

/// Copilot-backed subsystems configured from the top-level YAML file.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CopilotConfig {
    /// Configuration for the optional Copilot CLI executor.
    #[serde(default)]
    pub executor: ExecutorConfig,
    /// Configuration for the GitHub Copilot chat actor.
    #[serde(default)]
    pub copilot_chat: CopilotChatConfig,
}

/// Top-level application configuration loaded from YAML.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    /// All available LLM endpoints the user may choose from.
    pub endpoints: Vec<EndpointConfig>,
    /// The endpoint selected on startup if the user specifies none.
    pub default_endpoint: EndpointName,
    /// Agent conversation loop settings.
    pub agent: AgentConfig,
    /// Copilot executor and chat settings grouped into a single semantic section.
    #[serde(flatten)]
    pub copilot: CopilotConfig,
    /// Filesystem-backed persistence paths.
    ///
    /// Expected under the `persistence:` key in YAML (not flattened into
    /// top-level keys). Defaults to log_dir=`./logs` and sessions_dir=`None`
    /// when the `persistence:` section is absent from the config file.
    #[serde(default)]
    pub persistence: PersistenceConfig,
    /// Project-owned runtime behavior settings.
    #[serde(default)]
    pub program_settings: ProgramSettings,
    /// User preferences persisted across sessions.
    #[serde(default)]
    pub user_settings: UserSettings,
}

/// Look up an endpoint configuration by its name.
///
/// Performs a linear scan of `config.endpoints`. Returns a reference to the
/// first `EndpointConfig` whose `name` field matches `name`, or `None` if no
/// match exists. This is the single endpoint-lookup function - do not duplicate
/// this scan in actor loops or wiring code.
pub fn find_endpoint<'a>(config: &'a AppConfig, name: &EndpointName) -> Option<&'a EndpointConfig> {
    config.endpoints.iter().find(|ep| &ep.name == name)
}
