//! LlmActor spawn and run loop; dispatches completion requests as parallel tasks.

use super::handle::LlmHandle;
use super::llm_actor_ops as actor_ops;
use super::providers::openrouter_cache::build_openrouter_cache_headers;
use augur_domain::channels::LLM_COMMAND_CAPACITY;
use augur_domain::config::provider_catalog::{
    OpenRouterCacheConfig, default_provider_catalog_dir, load_provider_catalog,
};
use augur_domain::config::{AppConfig, Provider};
use augur_domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::types::{AgentOutput, Message, StreamChunk};
use augur_provider_shared::request_context::{
    CompleteFields, CompleteRoute, LlmCommand, RequestContext, RequestPayload,
    build_request_context,
};
use augur_provider_shared::{
    stream_anthropic_complete, stream_ollama_complete, stream_openai_complete,
};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

/// Runtime configuration bundle for the LLM actor run loop.
///
/// Bundles `AppConfig` and `OpenRouterCacheConfig` so the run loop and
/// dispatch helpers stay within the three-parameter limit.
pub(super) struct LlmRunConfig {
    pub(super) app: AppConfig,
    pub(super) or_cache: OpenRouterCacheConfig,
    /// Session UUID shared with the persistence layer.
    ///
    /// Forwarded as the `user` field in OpenAI-compatible request bodies and
    /// as `HTTP-Referer` + `X-OpenRouter-Title` in OpenRouter HTTP headers so requests appear attributed
    /// in the OpenRouter activity log.
    pub(super) session_id: String,
    /// Logger handle for routing raw LLM request/response bodies to the JSONL log.
    pub(super) logger: augur_domain::domain::actor_contracts::LoggerHandle,
}

/// Spawn the LLM actor task and return its join handle and communication handle.
///
/// The actor owns `config` - no Arc, no shared reference. Startup model
/// availability now comes from provider-YAML endpoint catalogs in TUI runtime
/// state, so this actor does not emit `AgentOutput::ModelsAvailable`. Each
/// `Complete` command is validated via `build_request_context` then dispatched
/// as an independent tokio task so the run loop is never blocked by network I/O.
#[tracing::instrument(skip_all, level = "info")]
pub fn spawn(
    config: AppConfig,
    agent_tx: broadcast::Sender<AgentOutput>,
    session_id: String,
    logger: augur_domain::domain::actor_contracts::LoggerHandle,
) -> (JoinHandle<()>, LlmHandle) {
    let _ = agent_tx;
    let or_cache = load_openrouter_cache_config();
    let (tx, rx) = mpsc::channel(*LLM_COMMAND_CAPACITY);
    let handle = LlmHandle::new(tx);
    let run_config = LlmRunConfig {
        app: config,
        or_cache,
        session_id,
        logger,
    };
    let join = tokio::spawn(run(run_config, rx));
    (join, handle)
}

/// Load the OpenRouter cache config from the provider catalog at startup.
///
/// Returns `OpenRouterCacheConfig::default()` (disabled) when the catalog
/// file is absent, malformed, or has no `openrouter.cache` block.
fn load_openrouter_cache_config() -> OpenRouterCacheConfig {
    let dir = default_provider_catalog_dir();
    let catalog = match load_provider_catalog(&dir, Provider::OpenRouter) {
        Ok(Some(c)) => c,
        _ => return OpenRouterCacheConfig::default(),
    };
    catalog.openrouter.map(|o| o.cache).unwrap_or_default()
}

/// Inject OpenRouter-specific headers and session metadata into `ctx`.
///
/// For OpenRouter endpoints this sets:
/// - Cache control headers (when caching is enabled)
/// - `X-OpenRouter-Title: augur-cli` so requests appear attributed in the activity log
/// - `ctx.session_id` so the `user` field is included in the request body
///
/// For all other providers this is a no-op.
pub(super) fn inject_openrouter_headers(
    ctx: &mut RequestContext,
    cfg: &OpenRouterCacheConfig,
    session_id: &str,
) {
    if ctx.endpoint.provider == Provider::OpenRouter {
        let mut headers = build_openrouter_cache_headers(cfg).0;
        headers.push(("X-OpenRouter-Title".to_string(), "augur-cli".to_string()));
        headers.push((
            "HTTP-Referer".to_string(),
            "https://github.com/Kenneth-Posey/augur-cli".to_string(),
        ));
        ctx.extra_request_headers = headers;
        ctx.session_id = Some(session_id.to_string());
    }
}

/// Dispatches an automated single-message LLM request, logging the endpoint on success.
///
/// Inputs: `fields` - pre-built request fields including endpoint, message, and reply
/// sender; `cfg` - LLM run configuration used to build the request context.
/// On context-build failure, logs a warning and drops the reply sender.
fn dispatch_automated(fields: CompleteFields, cfg: &LlmRunConfig) {
    let endpoint_str = fields.route.endpoint.to_string();
    match build_request_context(fields, &cfg.app) {
        Err(e) => tracing::warn!("send_automated context error: {e}"),
        Ok(mut ctx) => {
            inject_openrouter_headers(&mut ctx, &cfg.or_cache, &cfg.session_id);
            tracing::info!("automated message dispatched to endpoint {endpoint_str}");
            tokio::spawn(dispatch_request(ctx));
        }
    }
}

/// Dispatches a full LLM completion request, sending an error chunk on context failure.
///
/// Inputs: `fields` - pre-built request fields; `err_tx` - sender used only on the
/// error path to deliver a `StreamChunk::Error` to the caller; `cfg` - run config.
fn dispatch_complete(
    fields: CompleteFields,
    err_tx: mpsc::Sender<StreamChunk>,
    cfg: &LlmRunConfig,
) {
    actor_ops::dispatch_complete(fields, err_tx, cfg);
}

fn handle_send_automated(fields: CompleteFields, cfg: &LlmRunConfig) {
    dispatch_automated(fields, cfg);
}

fn handle_complete_command(fields: CompleteFields, cfg: &LlmRunConfig) {
    let err_tx = fields.reply_tx.clone();
    dispatch_complete(fields, err_tx, cfg);
}

fn build_automated_fields(
    text: OutputText,
    endpoint: augur_domain::EndpointName,
    reply_tx: mpsc::Sender<StreamChunk>,
) -> CompleteFields {
    let msg = Message::user(text.into_inner());
    CompleteFields::builder()
        .route(CompleteRoute::builder().endpoint(endpoint).build())
        .payload(
            RequestPayload::builder()
                .messages(vec![msg])
                .tools(vec![])
                .build(),
        )
        .reply_tx(reply_tx)
        .build()
}

async fn run(cfg: LlmRunConfig, mut rx: mpsc::Receiver<LlmCommand>) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            LlmCommand::Shutdown => break,
            LlmCommand::SendAutomated {
                text,
                endpoint,
                reply_tx,
            } => {
                let mut fields = build_automated_fields(text, endpoint, reply_tx);
                fields.logger = Some(cfg.logger.clone());
                handle_send_automated(fields, &cfg);
            }
            LlmCommand::Complete {
                endpoint,
                messages,
                tools,
                reply_tx,
                cache,
                model_override,
            } => handle_complete_command(
                CompleteFields::builder()
                    .route(
                        CompleteRoute::builder()
                            .endpoint(endpoint)
                            .maybe_model_override(model_override)
                            .build(),
                    )
                    .payload(
                        RequestPayload::builder()
                            .messages(messages)
                            .tools(tools)
                            .maybe_cache(cache)
                            .build(),
                    )
                    .reply_tx(reply_tx)
                    .logger(cfg.logger.clone())
                    .build(),
                &cfg,
            ),
        }
    }
}

/// Dispatch one streaming completion request to the selected provider adapter.
pub(super) async fn dispatch_request(ctx: RequestContext) {
    match ctx.endpoint.provider {
        Provider::OpenAi => stream_openai_complete(ctx).await,
        Provider::Anthropic => stream_anthropic_complete(ctx).await,
        Provider::Ollama => stream_ollama_complete(ctx).await,
        Provider::OpenRouter => super::providers::openrouter::stream_complete(ctx).await,
    }
}
