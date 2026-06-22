//! OpenAI-compatible streaming completion helpers shared by provider crates.

use crate::{
    request_context::{RequestContext, ToolDefinition, resolve_api_key},
    retry::{
        HTTP_RATE_LIMIT_STATUS, MAX_RETRY_ATTEMPTS, compute_backoff_wait, is_requests_exceeded,
        parse_retry_after,
    },
    streaming::{SseChunk, drain_complete_sse_lines},
};
use augur_domain::config::types::Provider;
use augur_domain::domain::newtypes::{Count, NumericNewtype, TokenCount};
use augur_domain::domain::string_newtypes::{
    AccumulatedText, BearerToken, ModelName, OutputText, StringNewtype, ToolCallId, ToolName,
};
use augur_domain::domain::types::{LlmTokenCounts, LlmUsage, Message, Role, StreamChunk};
use futures_util::StreamExt;

/// Streaming completion using the OpenAI API format.
///
/// Resolves the API key via `resolve_api_key` (direct `api_key` field takes
/// precedence over `api_key_env`) then delegates to `stream_openai_compat`.
/// Called by provider crates for `Provider::OpenAi` endpoints.
#[tracing::instrument(skip_all, fields(model = %ctx.endpoint.model))]
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
    stream_openai_compat(ctx, bearer).await;
}

pub use stream_complete as stream_openai_complete;

/// One tool call accumulation slot, indexed by position in the delta stream.
#[derive(bon::Builder)]
struct PendingToolCall {
    /// Provider-assigned id (filled from the first delta that carries it).
    id: Option<ToolCallId>,
    /// Tool name (filled from the first delta that carries it).
    name: Option<ToolName>,
    /// JSON argument string fragment, accumulated across multiple deltas.
    #[builder(default)]
    args_buf: String,
}

/// Mutable state accumulated across OpenAI SSE chunks for one request.
#[derive(bon::Builder)]
struct OpenAiStreamState {
    /// All in-progress tool calls indexed by their stream position.
    ///
    /// Replaces the single `pending_tool_name`/`pending_tool_args` pair so
    /// that parallel tool calls (multiple `tool_calls[N]` entries in one
    /// response) are accumulated correctly without argument fragments from
    /// different calls being mixed together.
    pending_tool_calls: Vec<PendingToolCall>,
    /// Model name reported by the provider stream.
    model: ModelName,
    /// Accumulated token counts from the provider's usage object.
    ///
    /// All four fields (`tokens_in`, `tokens_out`, `tokens_cached`,
    /// `cache_write_tokens`) are updated from the last SSE chunk that
    /// contains a `usage` object. `tokens_cached` comes from
    /// `prompt_tokens_details.cached_tokens` (OpenRouter / DeepSeek
    /// automatic caching).
    token_counts: LlmTokenCounts,
}

/// Request bundle for a retrying OpenAI-compatible POST.
#[derive(bon::Builder)]
struct OpenAiRetryRequest<'a> {
    /// Reply channel used for streamed status and error chunks.
    reply_tx: &'a tokio::sync::mpsc::Sender<StreamChunk>,
    /// Target provider URL.
    url: &'a str,
    /// Optional bearer token.
    bearer: Option<&'a str>,
    /// Serialized JSON request body.
    body_str: &'a str,
    /// Extra HTTP headers to inject beyond content-type and Authorization.
    ///
    /// Used by OpenRouter to send X-OpenRouter-Cache. Empty for OpenAI and Ollama.
    #[builder(default)]
    extra_headers: Vec<(String, String)>,
}

/// Mutable parsing state carried across streamed OpenAI chunks.
struct OpenAiChunkState<'a> {
    /// Trailing partial SSE line from the previous chunk.
    carry: &'a mut AccumulatedText,
    /// Accumulated stream usage and tool-call state.
    stream_state: &'a mut OpenAiStreamState,
}

/// Core OpenAI-compatible SSE streaming loop, callable with or without a bearer token.
///
/// Used by `stream_openai_complete` (OpenAI, with key) and `stream_ollama_complete`
/// (Ollama, no key). Builds a JSON body, serializes it once, then delegates to
/// `send_with_retry` to handle 429 rate-limit responses. On success, reads the
/// SSE byte stream and forwards `StreamChunk` events on `ctx.reply_tx` until
/// `[DONE]` or an error. Tool call name and arguments are accumulated across
/// multiple deltas. Emits `StreamChunk::Usage` before `StreamChunk::Done`.
#[tracing::instrument(skip_all, fields(model = %ctx.endpoint.model))]
pub async fn stream_openai_compat(ctx: RequestContext, bearer: Option<BearerToken>) {
    let Some(body_str) = serialize_openai_body(&ctx).await else {
        return;
    };
    tracing::debug!(
        target: "llm_raw",
        direction = "request",
        provider = %ctx.endpoint.provider,
        model = ctx.endpoint.model.as_str(),
    );
    let url = format!("{}/chat/completions", ctx.endpoint.base_url.as_str());
    let request = OpenAiRetryRequest::builder()
        .reply_tx(&ctx.reply_tx)
        .url(&url)
        .maybe_bearer(bearer.as_ref().map(BearerToken::as_str))
        .body_str(&body_str)
        .extra_headers(ctx.extra_request_headers.clone())
        .build();
    let response = match send_with_retry(request).await {
        Some(r) => r,
        None => return,
    };
    stream_openai_response(&ctx, response).await;
}

async fn serialize_openai_body(ctx: &RequestContext) -> Option<String> {
    match serde_json::to_string(&build_openai_body(ctx)) {
        Ok(body) => Some(body),
        Err(error) => {
            let _ = ctx
                .reply_tx
                .send(StreamChunk::Error(OutputText::new(error.to_string())))
                .await;
            None
        }
    }
}

async fn stream_openai_response(ctx: &RequestContext, response: reqwest::Response) {
    let mut stream = response.bytes_stream();
    let mut carry = AccumulatedText::from("");
    let mut state = OpenAiStreamState::builder()
        .pending_tool_calls(Vec::new())
        .model(ModelName::new(""))
        .token_counts(LlmTokenCounts::default())
        .build();
    while let Some(chunk_result) = stream.next().await {
        if process_openai_stream_chunk_result(
            chunk_result,
            ctx,
            &mut OpenAiChunkState {
                carry: &mut carry,
                stream_state: &mut state,
            },
        )
        .await
        {
            return;
        }
    }
    tracing::debug!(
        event = "provider_stream_end",
        end_reason = "http_eof",
        carry_len = carry.as_str().len(),
        pending_tool_calls = state.pending_tool_calls.len(),
        model_seen = !state.model.as_str().is_empty(),
    );
    if should_process_carry_line(&carry)
        && process_openai_line(carry.as_str(), ctx, &mut state).await
    {
        return;
    }
    finish_stream(&state, ctx).await;
}

fn should_process_carry_line(carry: &AccumulatedText) -> bool {
    !carry.as_str().is_empty()
}

async fn process_openai_stream_chunk_result<B: AsRef<[u8]>>(
    chunk_result: Result<B, reqwest::Error>,
    ctx: &RequestContext,
    chunk_state: &mut OpenAiChunkState<'_>,
) -> bool {
    let chunk = match chunk_result {
        Ok(chunk) => chunk,
        Err(error) => {
            let _ = ctx
                .reply_tx
                .send(StreamChunk::Error(OutputText::new(error.to_string())))
                .await;
            return true;
        }
    };
    process_openai_chunk(chunk.as_ref(), ctx, chunk_state).await
}

async fn process_openai_chunk(
    chunk: &[u8],
    ctx: &RequestContext,
    chunk_state: &mut OpenAiChunkState<'_>,
) -> bool {
    for line in drain_complete_sse_lines(chunk_state.carry, SseChunk::from(chunk)) {
        if process_openai_line(&line, ctx, chunk_state.stream_state).await {
            return true;
        }
    }
    false
}

async fn process_openai_line(
    line: &str,
    ctx: &RequestContext,
    state: &mut OpenAiStreamState,
) -> bool {
    if line.trim_end() == "data: [DONE]" {
        tracing::debug!(
            event = "provider_stream_end",
            end_reason = "done_marker",
            pending_tool_calls = state.pending_tool_calls.len(),
            model_seen = !state.model.as_str().is_empty(),
        );
        finish_stream(state, ctx).await;
        return true;
    }
    if let Some(stripped) = line.strip_prefix("data: ") {
        accumulate_openai_delta(stripped, ctx, state).await;
    } else {
        tracing::debug!(
            event = "provider_sse_line_ignored",
            reason = "missing_data_prefix",
            line_len = line.len(),
        );
    }
    false
}

/// Send an OpenAI-compatible request with automatic 429 retry.
///
/// Attempts the POST up to `MAX_RETRY_ATTEMPTS` times. On HTTP 429, reads the
/// `Retry-After` header via `parse_retry_after`, sends `StreamChunk::RateLimitRetry`
/// to notify the TUI, sleeps, then retries. On other non-2xx responses, sends
/// `StreamChunk::Error` and returns `None`. Returns `Some(response)` on success.
async fn send_with_retry(request: OpenAiRetryRequest<'_>) -> Option<reqwest::Response> {
    let client = reqwest::Client::new();
    for attempt in 0..MAX_RETRY_ATTEMPTS {
        let response = send_openai_request(&client, &request).await?;
        let Some(response) = handle_openai_rate_limit(attempt, response, request.reply_tx).await
        else {
            continue;
        };
        if response.status().is_success() {
            return Some(response);
        }
        if emit_openai_http_error(response, request.reply_tx).await {
            return None;
        }
    }
    let _ = request
        .reply_tx
        .send(StreamChunk::Error(OutputText::new(format!(
            "rate limit: exhausted {} retries",
            MAX_RETRY_ATTEMPTS
        ))))
        .await;
    None
}

async fn send_openai_request(
    client: &reqwest::Client,
    request: &OpenAiRetryRequest<'_>,
) -> Option<reqwest::Response> {
    let mut req = client
        .post(request.url)
        .header("content-type", "application/json")
        .body(request.body_str.to_owned());
    if let Some(key) = request.bearer {
        req = req.bearer_auth(key);
    }
    for (k, v) in &request.extra_headers {
        req = req.header(k, v);
    }
    match req.send().await {
        Ok(response) => Some(response),
        Err(error) => {
            let _ = request
                .reply_tx
                .send(StreamChunk::Error(OutputText::new(error.to_string())))
                .await;
            None
        }
    }
}

async fn handle_openai_rate_limit(
    attempt: usize,
    response: reqwest::Response,
    reply_tx: &tokio::sync::mpsc::Sender<StreamChunk>,
) -> Option<reqwest::Response> {
    if response.status().as_u16() != HTTP_RATE_LIMIT_STATUS {
        return Some(response);
    }
    let header_wait = parse_retry_after(&response);
    let body = response.text().await.unwrap_or_default();
    let wait = if is_requests_exceeded(&OutputText::from(body.as_str())) {
        compute_backoff_wait(Count::new(attempt))
    } else {
        header_wait
    };
    tracing::warn!(
        attempt,
        wait_secs = wait.inner(),
        "OpenAI rate limit - retrying"
    );
    let _ = reply_tx.send(StreamChunk::RateLimitRetry(wait)).await;
    tokio::time::sleep(std::time::Duration::from_secs(wait.inner())).await;
    None
}

async fn emit_openai_http_error(
    response: reqwest::Response,
    reply_tx: &tokio::sync::mpsc::Sender<StreamChunk>,
) -> bool {
    if response.status().is_success() {
        return false;
    }
    let status = response.status().as_u16();
    let body_text = response.text().await.unwrap_or_default();
    let _ = reply_tx
        .send(StreamChunk::Error(OutputText::new(format!(
            "HTTP {status}: {body_text}"
        ))))
        .await;
    true
}

/// Build the `LlmUsage` from accumulated stream state and request context.
///
/// Called immediately before emitting `StreamChunk::Done`. The model name comes
/// from the first chunk that includes the `model` field; token counts (including
/// cached tokens from `prompt_tokens_details.cached_tokens`) come from the final
/// chunk that includes the `usage` object (enabled by
/// `stream_options.include_usage`). Falls back to the endpoint's configured
/// model name when no SSE event carried a `"model"` field.
fn build_usage_chunk(state: &OpenAiStreamState, ctx: &RequestContext) -> LlmUsage {
    let model_name = if state.model.as_str().is_empty() {
        ModelName::new(ctx.endpoint.model.as_str())
    } else {
        state.model.clone()
    };
    LlmUsage {
        model: OutputText::new(model_name.as_str()),
        token_counts: state.token_counts.clone(),
        temperature: ctx.params.temperature,
    }
}

/// Log a structured response summary to the `llm_raw` target.
fn log_llm_response(ctx: &RequestContext, usage: &LlmUsage) {
    tracing::debug!(
        target: "llm_raw",
        direction = "response",
        provider = %ctx.endpoint.provider,
        model = usage.model.as_str(),
        tokens_in = usage.tokens_in.inner(),
        tokens_out = usage.tokens_out.inner(),
        tokens_cached = usage.tokens_cached.inner(),
        cache_write_tokens = usage.cache_write_tokens.inner(),
    );
}

/// Finalize the stream: log the response, emit `Usage` and `Done` chunks.
///
/// Called from both the normal `[DONE]` path and the fallback path at the end
/// of `stream_openai_response` so both exit points produce identical behavior.
async fn finish_stream(state: &OpenAiStreamState, ctx: &RequestContext) {
    let usage = build_usage_chunk(state, ctx);
    log_llm_response(ctx, &usage);
    let _ = ctx.reply_tx.send(StreamChunk::Usage(usage)).await;
    let _ = ctx.reply_tx.send(StreamChunk::Done).await;
}

/// Process one OpenAI SSE data line, emitting chunks and accumulating tool and usage state.
///
/// Text tokens are emitted immediately via `Token`. Tool call name fragments are
/// saved in `state.pending_tool_name`; argument fragments are appended to
/// `state.pending_tool_args`. When `finish_reason == "tool_calls"` is seen the
/// accumulated `ToolCall` chunk is emitted and state is reset for the next call.
/// The `model` field and `usage` object (from `stream_options.include_usage`) are
/// captured into `state` for use when building the final `Usage` chunk.
async fn accumulate_openai_delta(data: &str, ctx: &RequestContext, state: &mut OpenAiStreamState) {
    let Ok(val) = serde_json::from_str::<serde_json::Value>(data) else {
        tracing::warn!(
            event = "provider_delta_parse_failed",
            payload_len = data.len(),
        );
        return;
    };
    update_openai_usage(&val, state);
    let choice = &val["choices"][0];
    let delta = &choice["delta"];
    emit_openai_text(delta, &ctx.reply_tx).await;
    accumulate_openai_tool_call(delta, state);
    emit_openai_tool_call(choice, ctx, state).await;
}

fn update_openai_usage(val: &serde_json::Value, state: &mut OpenAiStreamState) {
    update_openai_model(val, state);
    update_openai_token_counts(val, &mut state.token_counts);
}

fn update_openai_model(val: &serde_json::Value, state: &mut OpenAiStreamState) {
    if let Some(model) = val["model"].as_str().filter(|model| !model.is_empty()) {
        state.model = ModelName::new(model);
    }
}

fn update_openai_token_counts(val: &serde_json::Value, token_counts: &mut LlmTokenCounts) {
    update_prompt_tokens(val, token_counts);
    update_completion_tokens(val, token_counts);
    update_cached_prompt_tokens(val, token_counts);
    update_cache_write_tokens(val, token_counts);
}

fn update_prompt_tokens(val: &serde_json::Value, token_counts: &mut LlmTokenCounts) {
    if let Some(tokens) = val["usage"]["prompt_tokens"].as_u64() {
        token_counts.tokens_in = TokenCount::new(tokens);
    }
}

fn update_completion_tokens(val: &serde_json::Value, token_counts: &mut LlmTokenCounts) {
    if let Some(tokens) = val["usage"]["completion_tokens"].as_u64() {
        token_counts.tokens_out = TokenCount::new(tokens);
    }
}

fn update_cached_prompt_tokens(val: &serde_json::Value, token_counts: &mut LlmTokenCounts) {
    if let Some(tokens) = val["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64() {
        token_counts.tokens_cached = TokenCount::new(tokens);
    }
}

fn update_cache_write_tokens(val: &serde_json::Value, token_counts: &mut LlmTokenCounts) {
    if let Some(tokens) = val["usage"]["prompt_tokens_details"]["cache_write_tokens"].as_u64() {
        token_counts.cache_write_tokens = TokenCount::new(tokens);
    }
}

async fn emit_openai_text(
    delta: &serde_json::Value,
    reply_tx: &tokio::sync::mpsc::Sender<StreamChunk>,
) {
    if let Some(text) = delta["content"].as_str().filter(|text| !text.is_empty()) {
        let _ = reply_tx
            .send(StreamChunk::Token(OutputText::new(text)))
            .await;
    }
}

/// Accumulate tool call deltas from one SSE data object into `state`.
///
/// Each chunk may carry one or more `tool_calls[N]` entries. The `index`
/// field within each entry identifies which parallel tool call the fragment
/// belongs to. The slot is grown on demand so out-of-order or sparse indices
/// are handled safely.
fn accumulate_openai_tool_call(delta: &serde_json::Value, state: &mut OpenAiStreamState) {
    let Some(arr) = delta["tool_calls"].as_array() else {
        return;
    };
    for entry in arr {
        let idx = openai_tool_call_index(entry);
        let pending = pending_tool_call_slot(&mut state.pending_tool_calls, idx);
        merge_tool_call_entry(entry, pending);
        let fragment_len = entry["function"]["arguments"]
            .as_str()
            .map(|s| s.len())
            .unwrap_or(0);
        tracing::debug!(
            event = "tool_call_args_fragment",
            tool_index = idx,
            fragment_len,
            args_buf_len_total = pending.args_buf.len(),
            id_present = pending.id.is_some(),
            name_present = pending.name.is_some(),
        );
    }
}

fn openai_tool_call_index(entry: &serde_json::Value) -> usize {
    entry["index"].as_u64().unwrap_or(0) as usize
}

fn pending_tool_call_slot(
    pending_tool_calls: &mut Vec<PendingToolCall>,
    idx: usize,
) -> &mut PendingToolCall {
    while pending_tool_calls.len() <= idx {
        pending_tool_calls.push(PendingToolCall::builder().build());
    }
    &mut pending_tool_calls[idx]
}

fn merge_tool_call_entry(entry: &serde_json::Value, pending: &mut PendingToolCall) {
    assign_tool_call_id(entry, pending);
    let function = &entry["function"];
    assign_tool_call_name(function, pending);
    append_tool_call_arguments(function, pending);
}

fn assign_tool_call_id(entry: &serde_json::Value, pending: &mut PendingToolCall) {
    if let Some(id) = entry["id"].as_str().filter(|s| !s.is_empty()) {
        pending.id = Some(ToolCallId::new(id));
    }
}

fn assign_tool_call_name(function: &serde_json::Value, pending: &mut PendingToolCall) {
    if let Some(name) = function["name"].as_str().filter(|s| !s.is_empty()) {
        pending.name = Some(ToolName::new(name));
    }
}

fn append_tool_call_arguments(function: &serde_json::Value, pending: &mut PendingToolCall) {
    if let Some(args) = function["arguments"].as_str() {
        pending.args_buf.push_str(args);
    }
}

/// Emit one `StreamChunk::ToolCall` per accumulated tool call when the stream signals completion.
///
/// Only fires when `finish_reason == "tool_calls"`. All accumulated slots are
/// drained and emitted in index order; slots without a name are skipped.
async fn emit_openai_tool_call(
    choice: &serde_json::Value,
    ctx: &RequestContext,
    state: &mut OpenAiStreamState,
) {
    let finish_reason = choice["finish_reason"].as_str().unwrap_or("");
    if finish_reason != "tool_calls" {
        return;
    }
    for pending in state.pending_tool_calls.drain(..) {
        let Some(name) = pending.name else {
            continue;
        };
        let id = pending.id.unwrap_or_else(|| ToolCallId::new(""));
        let parse_result = serde_json::from_str::<serde_json::Value>(&pending.args_buf);
        let (arguments, args_parse_ok) = match parse_result {
            Ok(value) => (value, true),
            Err(_) => (serde_json::Value::String(pending.args_buf.clone()), false),
        };
        let arguments_kind = json_value_kind(&arguments);
        if let Some(logger) = &ctx.logger {
            logger.log_llm_raw(
                "tool_call",
                &ctx.endpoint.provider.to_string(),
                state.model.as_str(),
                pending.args_buf.clone(),
            );
        }
        tracing::debug!(
            target: "llm_raw",
            direction = "tool_call",
            model = state.model.as_str(),
            tool_name = name.as_str(),
        );
        tracing::debug!(
            event = "tool_call_emitted",
            finish_reason,
            tool_name = name.as_str(),
            tool_id_empty = id.as_str().is_empty(),
            args_buf_len = pending.args_buf.len(),
            args_empty = pending.args_buf.is_empty(),
            args_parse_ok,
            args_json_kind = arguments_kind,
        );
        let _ = ctx
            .reply_tx
            .send(StreamChunk::ToolCall {
                id,
                name,
                arguments,
            })
            .await;
    }
}

fn json_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Build the inner `tool_calls` JSON array for an assistant message.
///
/// Maps each `ToolCall` to the `{"id","type","function":{"name","arguments"}}` shape
/// required by the OpenAI wire format. Extracted from `to_openai_messages` to keep
/// that function within the 50-line limit and to allow independent testing.
fn tool_calls_json(calls: &[augur_domain::domain::types::ToolCall]) -> Vec<serde_json::Value> {
    calls
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id.as_str(),
                "type": "function",
                "function": {
                    "name": c.name.as_str(),
                    "arguments": c.arguments.to_string(),
                }
            })
        })
        .collect()
}

/// Serialise a `Role::Assistant` message that contains tool calls.
///
/// Emits `"content": null` when the assistant text is empty, satisfying the
/// OpenAI requirement that the content field is present but null for pure
/// tool-call assistant turns. Extracted from `to_openai_messages`.
fn tool_call_assistant_message_json(msg: &Message) -> serde_json::Value {
    let calls_json = if let Some(ref calls) = msg.tool_calls {
        tool_calls_json(calls)
    } else {
        vec![]
    };
    let content = if msg.content.as_str().is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::String(msg.content.as_str().to_owned())
    };
    serde_json::json!({
        "role": "assistant",
        "content": content,
        "tool_calls": calls_json,
    })
}

/// Convert domain `Message` slice to the OpenAI `messages` array JSON shape.
///
/// Maps each message to the OpenAI Chat Completions wire format:
/// - `Role::Tool` messages include `"tool_call_id"` when `msg.tool_call_id` is set.
/// - `Role::Assistant` messages include `"tool_calls"` array and `"content": null`
///   when `msg.tool_calls` is set, so providers can correlate tool results.
fn to_openai_messages(messages: &[Message]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = messages.iter().map(to_openai_message).collect();
    serde_json::Value::Array(arr)
}

fn to_openai_message(msg: &Message) -> serde_json::Value {
    if msg.role == Role::Tool {
        return to_openai_tool_message(msg);
    }
    if msg.role == Role::Assistant && msg.tool_calls.is_some() {
        return tool_call_assistant_message_json(msg);
    }
    to_openai_standard_message(msg)
}

fn to_openai_tool_message(msg: &Message) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "role": "tool",
        "content": msg.content.as_str(),
    });
    if let Some(ref id) = msg.tool_call_id {
        obj["tool_call_id"] = serde_json::Value::String(id.as_str().to_owned());
    }
    obj
}

fn to_openai_standard_message(msg: &Message) -> serde_json::Value {
    serde_json::json!({
        "role": openai_role_name(msg.role.clone()),
        "content": msg.content.as_str()
    })
}

fn openai_role_name(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

/// Convert `ToolDefinition` slice to the OpenAI `tools` array JSON shape.
///
/// Each tool maps to the `{"type":"function","function":{...}}` envelope
/// required by the OpenAI function-calling API.
fn to_openai_tools(tools: &[ToolDefinition]) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name.as_str(),
                    "description": &t.description,
                    "parameters": &t.parameters,
                }
            })
        })
        .collect();
    serde_json::Value::Array(arr)
}

/// Build the OpenAI Chat Completions API request body from a `RequestContext`.
///
/// Omits `tools` when the tools list is empty - OpenAI rejects `"tools": []`
/// with a 400 error. `max_tokens` and `temperature` are sourced from
/// `ctx.params` (set from agent config) so the values are always consistent
/// with the runtime configuration.
fn build_openai_body(ctx: &RequestContext) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert("model".into(), ctx.endpoint.model.as_str().into());
    body.insert("messages".into(), to_openai_messages(&ctx.payload.messages));
    body.insert("stream".into(), true.into());
    body.insert(
        "stream_options".into(),
        serde_json::json!({"include_usage": true}),
    );
    body.insert("max_tokens".into(), ctx.params.max_tokens.inner().into());
    body.insert("temperature".into(), ctx.params.temperature.inner().into());
    if !ctx.payload.tools.is_empty() {
        body.insert("tools".into(), to_openai_tools(&ctx.payload.tools));
    }
    if let Some(ref session_id) = ctx.session_id {
        let key = if ctx.endpoint.provider == Provider::OpenRouter {
            "session_id"
        } else {
            "user"
        };
        body.insert(key.into(), session_id.clone().into());
    }
    serde_json::Value::Object(body)
}
