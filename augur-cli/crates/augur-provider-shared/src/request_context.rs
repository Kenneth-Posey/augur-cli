//! LLM request context, commands, and API-key resolution for provider crates.

use augur_domain::config::types::{AppConfig, EndpointConfig, find_endpoint};
use augur_domain::domain::newtypes::{Temperature, TokenCount};
use augur_domain::domain::string_newtypes::ModelId;
use augur_domain::domain::types::{CacheSnapshot, Message, StreamChunk};
use augur_domain::domain::{ApiKeyValue, EndpointName, EnvVarName, OutputText, StringNewtype};
use std::fmt;
use tokio::sync::mpsc;

pub use augur_domain::tools::definition::ToolDefinition;

/// A streaming completion request to be processed by the LLM actor.
///
/// Variants flow through the `mpsc::channel<LlmCommand>` from `LlmHandle`
/// to the actor task. Each `Complete` variant carries its own reply sender
/// so responses flow back to the caller with no shared state.
pub enum LlmCommand {
    /// Submit a completion request. `reply_tx` receives `StreamChunk` events
    /// until `StreamChunk::Done` or `StreamChunk::Error` signals end-of-stream.
    Complete {
        endpoint: EndpointName,
        messages: Vec<Message>,
        tools: Vec<ToolDefinition>,
        reply_tx: mpsc::Sender<StreamChunk>,
        /// Optional cache tiers for Anthropic system message injection.
        cache: Option<CacheSnapshot>,
        /// Optional model override for this request.
        model_override: Option<ModelId>,
    },
    /// Submit a lightweight automated user message to the LLM.
    ///
    /// Fires a one-shot request from an automated feed. The caller supplies
    /// `reply_tx`; the actor uses it exactly like `Complete`'s `reply_tx` so
    /// the response stream flows back to the caller rather than being silently
    /// dropped. Callers that need to render tokens should wire the returned
    /// receiver through `forward_reply_to_broadcast` in the wiring layer.
    SendAutomated {
        /// Text content of the automated user message.
        text: OutputText,
        /// Endpoint to route the message through.
        endpoint: EndpointName,
        /// Per-request reply sender. The actor passes it to the provider task;
        /// the provider streams `StreamChunk` events until `Done` or `Error`.
        reply_tx: mpsc::Sender<StreamChunk>,
    },
    /// Gracefully stop the actor task loop.
    Shutdown,
}

/// Bundles `LlmCommand::Complete` fields for passing to `build_request_context`.
///
/// Avoids destructuring the command in multiple places. Consumed entirely by
/// `build_request_context`; on error the `reply_tx` inside is dropped.
#[derive(bon::Builder)]
pub struct CompleteFields {
    /// Route selection for the request.
    pub route: CompleteRoute,
    /// Bundled message/tool/cache payload.
    pub payload: RequestPayload,
    /// Per-request reply sender. Dropped on error so the receiver closes cleanly.
    pub reply_tx: mpsc::Sender<StreamChunk>,
    /// Optional logger handle for routing raw LLM bodies to the JSONL log.
    pub logger: Option<augur_domain::domain::actor_contracts::LoggerHandle>,
}

/// Route-level request fields for endpoint/model selection.
#[derive(bon::Builder)]
pub struct CompleteRoute {
    /// Requested endpoint name.
    pub endpoint: EndpointName,
    /// Optional model override. When set, overrides the endpoint model.
    pub model_override: Option<ModelId>,
}

/// Groups message, tool, and cache data for a single LLM request.
///
/// Extracted from `RequestContext` to satisfy the 5-field struct limit.
/// Consumed by providers to build the request body; `cache` is only used
/// by the Anthropic provider (OpenAI ignores it).
#[derive(bon::Builder)]
pub struct RequestPayload {
    /// Full message history for the request.
    pub messages: Vec<Message>,
    /// Tool schemas available to the model.
    pub tools: Vec<ToolDefinition>,
    /// Tiered file content for Anthropic `cache_control` injection.
    /// `None` when no working file is set or the project has no deps.
    pub cache: Option<CacheSnapshot>,
}

/// LLM generation parameters forwarded to every provider.
///
/// Populated from `AppConfig.agent` in `build_request_context` so that
/// `max_tokens` and `temperature` are always included in the request body.
/// Both providers read these from `RequestContext.params` - do not hardcode
/// generation parameters in the provider modules.
pub struct GenerationParams {
    /// Maximum tokens the LLM may generate per response.
    pub max_tokens: TokenCount,
    /// Sampling temperature. Higher values produce more varied output.
    pub temperature: Temperature,
}

/// Validated, resolved completion request ready for provider dispatch.
///
/// Created by `build_request_context` after endpoint lookup. The `api_key`
/// is intentionally absent - providers read it from env at dispatch time so
/// secrets are never stored in long-lived structs.
#[derive(bon::Builder)]
pub struct RequestContext {
    /// Resolved endpoint configuration for the request.
    pub endpoint: EndpointConfig,
    /// Bundled message, tool, and cache payload for this request.
    pub payload: RequestPayload,
    /// Per-request channel sender; the provider streams chunks to this sender.
    pub reply_tx: mpsc::Sender<StreamChunk>,
    /// Generation parameters sourced from agent config.
    pub params: GenerationParams,
    /// Extra HTTP headers to inject on the outgoing request.
    ///
    /// Populated for OpenRouter endpoints when response caching is enabled.
    /// Empty for all other providers - no behavior change.
    #[builder(default)]
    pub extra_request_headers: Vec<(String, String)>,
    /// Session identifier forwarded as the OpenAI `user` field.
    ///
    /// Populated from the app session UUID so requests are attributable in
    /// OpenRouter's activity log. Optional so providers that don't use it
    /// can ignore it without any code changes.
    pub session_id: Option<String>,
    /// Optional logger handle for routing raw LLM bodies to the JSONL message log.
    ///
    /// When `Some`, provider functions call `logger.log_llm_raw(...)` instead of
    /// writing the full request body to the trace log. `None` for callers that do
    /// not supply a logger (e.g., tests, automated paths without wiring).
    pub logger: Option<augur_domain::domain::actor_contracts::LoggerHandle>,
}

/// Errors produced before a request reaches a provider.
///
/// Sent as `StreamChunk::Error` on the reply channel when building context fails.
#[derive(Debug)]
pub enum LlmError {
    /// No endpoint in config matches the requested name.
    UnknownEndpoint(EndpointName),
    /// The env var named as `api_key_env` is not set.
    MissingApiKey(EnvVarName),
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LlmError::UnknownEndpoint(name) => write!(f, "unknown endpoint: {name}"),
            LlmError::MissingApiKey(var) => write!(f, "missing API key env var: {var}"),
        }
    }
}

impl std::error::Error for LlmError {}

/// Validate command fields against config and produce a `RequestContext`.
///
/// Looks up the endpoint by name in `config`; returns `UnknownEndpoint` if
/// absent. When the endpoint has no `api_key` set, checks that the required
/// API key env var exists (if any); returns `MissingApiKey` if absent. Called
/// by `LlmActor`'s run loop before spawning a provider task.
pub fn build_request_context(
    fields: CompleteFields,
    config: &AppConfig,
) -> Result<RequestContext, LlmError> {
    let CompleteFields {
        route,
        payload,
        reply_tx,
        logger,
    } = fields;
    let mut endpoint = find_endpoint(config, &route.endpoint)
        .ok_or_else(|| LlmError::UnknownEndpoint(route.endpoint.clone()))?
        .clone();
    resolve_api_key(&endpoint).map_err(LlmError::MissingApiKey)?;

    if let Some(model_override) = route.model_override {
        endpoint.model = model_override.as_str().into();
    }

    Ok(RequestContext::builder()
        .endpoint(endpoint)
        .payload(payload)
        .reply_tx(reply_tx)
        .params(GenerationParams {
            max_tokens: config.agent.max_tokens,
            temperature: config.agent.temperature,
        })
        .maybe_logger(logger)
        .build())
}

/// Resolve the API key for an endpoint.
///
/// Returns the direct `api_key` value when set. Otherwise reads the env var
/// named by `api_key_env`. Returns an empty string for unauthenticated
/// endpoints (neither field set). Returns `Err(var_name)` when `api_key_env`
/// names a variable that is not present in the environment. Called by
/// `build_request_context` for preflight validation and by providers at
/// dispatch time to obtain the key value.
pub fn resolve_api_key(endpoint: &EndpointConfig) -> Result<ApiKeyValue, EnvVarName> {
    if let Some(ref key) = endpoint.credentials.api_key {
        return Ok(ApiKeyValue::new(key.as_str()));
    }
    match &endpoint.credentials.api_key_env {
        Some(var) => std::env::var(var.as_str())
            .map(ApiKeyValue::new)
            .map_err(|_| EnvVarName::new(var.as_str())),
        None => Ok(ApiKeyValue::new("")),
    }
}
