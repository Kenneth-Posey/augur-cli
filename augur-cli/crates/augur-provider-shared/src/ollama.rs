//! Ollama streaming completion helpers shared by provider crates.

use crate::{openai::stream_openai_compat, request_context::RequestContext};

/// Streaming completion for a local Ollama instance.
///
/// Ollama mirrors the OpenAI Chat Completions API at `/v1/chat/completions`.
/// No API key is used. Delegates to `stream_openai_compat(ctx, None)`.
#[tracing::instrument(skip_all, fields(model = %ctx.endpoint.model))]
pub async fn stream_complete(ctx: RequestContext) {
    stream_openai_compat(ctx, None).await;
}

pub use stream_complete as stream_ollama_complete;
