//! OpenRouter streaming completion provider.
//!
//! OpenRouter exposes an OpenAI-compatible API at `https://openrouter.ai/api/v1`.
//! This provider resolves a bearer token from the endpoint credentials, then
//! delegates entirely to the shared `stream_openai_compat` helper.  No custom SSE parsing
//! is required - the response format is identical to the OpenAI Chat Completions
//! streaming format.

use augur_domain::string_newtypes::{BearerToken, OutputText, StringNewtype};
use augur_domain::types::StreamChunk;
use augur_provider_shared::request_context::{RequestContext, resolve_api_key};

/// Streaming completion for an OpenRouter endpoint.
///
/// Resolves the bearer token via `resolve_api_key`.  On a missing env-var
/// error the function emits a `StreamChunk::Error` and returns early.  On
/// success it delegates to the shared OpenAI-compatible stream implementation.
#[tracing::instrument(skip_all, fields(provider = "openrouter"))]
pub async fn stream_complete(ctx: RequestContext) {
    let bearer = match resolve_api_key(&ctx.endpoint) {
        Ok(key) if key.is_empty() => None,
        Ok(key) => Some(BearerToken::new(key.into_inner())),
        Err(var) => {
            let _ = ctx
                .reply_tx
                .send(StreamChunk::Error(OutputText::new(format!(
                    "missing API key env var: {var}"
                ))))
                .await;
            return;
        }
    };
    augur_provider_shared::stream_openai_compat(ctx, bearer).await;
}
