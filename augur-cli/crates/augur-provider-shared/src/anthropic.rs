//! Anthropic Claude streaming completion helpers shared by provider crates.

mod body;
mod retry;

use crate::{
    request_context::{resolve_api_key, RequestContext},
    streaming::{drain_complete_sse_lines, SseChunk},
};
use augur_domain::domain::newtypes::{NumericNewtype, TokenCount};
use augur_domain::domain::string_newtypes::{
    AccumulatedText, ApiKeyValue, ModelName, OutputText, StringNewtype, ToolCallId, ToolName,
};
use augur_domain::domain::types::{LlmTokenCounts, LlmUsage, StreamChunk};
use futures_util::StreamExt;

/// Bundles three mutable stream state parameters into a buffer struct for accumulating
/// text and parsing state during LLM provider stream processing.
///
/// This type reduces the parameter count of `process_stream_lines` from 5 to 3 by grouping
/// the three mutable fields that represent stream accumulation state into a single value object.
///
/// # Invariants
///
/// - `carry` must not exceed max_buffer_size (e.g., 64KB)
/// - `current_event_type` must be one of: "text", "tool_call", "stop", "error"
/// - `state` must be in a valid EventParseState variant
/// - No concurrent access to buffer fields (exclusive mutable borrow)
///
/// # Example
///
/// ```ignore
/// let mut buffer = AccumulationBuffer {
///     carry: AccumulatedText::new(),
///     current_event_type: String::from("text"),
///     state: EventParseState::Start,
/// };
/// process_stream_lines(chunk, ctx, &mut buffer)?;
/// ```
#[derive(Debug)]
pub struct AccumulationBuffer {
    /// Partial text carried over from prior chunk
    pub carry: AccumulatedText,
    /// Current streaming event type classification
    pub current_event_type: String,
    /// FSM state for event parsing
    pub state: EventParseState,
}

impl AccumulationBuffer {
    /// Creates a new AccumulationBuffer with empty/start state.
    pub fn new() -> Self {
        AccumulationBuffer {
            carry: AccumulatedText::from(""),
            current_event_type: String::new(),
            state: EventParseState::empty(),
        }
    }
}

impl Default for AccumulationBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Streaming completion using the Anthropic Messages API.
///
/// Resolves the API key, serializes the request body once, then delegates to
/// `send_with_retry` to handle 429 rate-limit responses by waiting and
/// retrying up to `MAX_RETRY_ATTEMPTS` times. On success, streams events via
/// `stream_anthropic_events`. Sends `StreamChunk` events on `ctx.reply_tx`.
#[tracing::instrument(skip_all, fields(model = %ctx.endpoint.model))]
pub async fn stream_complete(ctx: RequestContext) {
    let Some(api_key) = resolve_api_key_or_emit(&ctx).await else {
        return;
    };
    let Some(body_str) = serialize_request_body_or_emit(&ctx).await else {
        return;
    };
    tracing::debug!(
        target: "llm_raw",
        direction = "request",
        provider = "anthropic",
        model = ctx.endpoint.model.as_str(),
    );
    let url = format!("{}/messages", ctx.endpoint.base_url.as_str());
    let request = retry::AnthropicRetryRequest::builder()
        .reply_tx(&ctx.reply_tx)
        .url(&url)
        .api_key(&api_key)
        .body_str(&body_str)
        .build();
    let Some(response) = retry::send_with_retry(request).await else {
        return;
    };
    stream_anthropic_events(ctx, response).await;
}

pub use stream_complete as stream_anthropic_complete;

async fn resolve_api_key_or_emit(ctx: &RequestContext) -> Option<ApiKeyValue> {
    match resolve_api_key(&ctx.endpoint) {
        Ok(key) => Some(key),
        Err(var) => {
            let _ = ctx
                .reply_tx
                .send(StreamChunk::Error(OutputText::new(format!(
                    "missing API key env var: {var}"
                ))))
                .await;
            None
        }
    }
}

async fn serialize_request_body_or_emit(ctx: &RequestContext) -> Option<String> {
    match serde_json::to_string(&build_anthropic_body(ctx)) {
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

/// Usage fields accumulated across Anthropic SSE events for one request.
#[derive(bon::Builder, Debug)]
struct AnthropicUsageAccum {
    /// Model name reported by the provider stream.
    model: ModelName,
    /// Prompt token count reported by the provider stream.
    tokens_in: TokenCount,
    /// Completion token count reported by the provider stream.
    tokens_out: TokenCount,
    /// Cache-read token count (`cache_read_input_tokens`).
    tokens_cached: TokenCount,
    /// Cache-write token count (`cache_creation_input_tokens`).
    #[builder(default)]
    cache_write_tokens: TokenCount,
}

/// Pending tool-call state accumulated across `content_block_*` SSE events.
///
/// Holds the tool id, name, and accumulating JSON argument string between the
/// `content_block_start` (which carries the id and name) and `content_block_stop`
/// (which triggers the `ToolCall` emit).
#[derive(bon::Builder, Debug)]
struct ToolCallState {
    pending_id: Option<ToolCallId>,
    pending_name: Option<ToolName>,
    #[builder(default)]
    pending_args: String,
}

/// Combined mutable state threaded through each SSE event handler call.
///
/// Bundles `AnthropicUsageAccum` (model and token fields) with `ToolCallState`
/// (tool-call JSON accumulation) so `handle_anthropic_event` stays within the
/// 3-parameter limit.
#[derive(Debug)]
pub struct EventParseState {
    usage: AnthropicUsageAccum,
    tool_call: ToolCallState,
}

impl EventParseState {
    fn empty() -> Self {
        EventParseState {
            usage: AnthropicUsageAccum::builder()
                .model(ModelName::new(""))
                .tokens_in(TokenCount::ZERO)
                .tokens_out(TokenCount::ZERO)
                .tokens_cached(TokenCount::ZERO)
                .cache_write_tokens(TokenCount::ZERO)
                .build(),
            tool_call: ToolCallState::builder().build(),
        }
    }
}

/// A single parsed SSE event: the event type line and its associated data line.
struct SseEvent<'a> {
    event_type: &'a str,
    data: &'a str,
}

/// Mutable parse state carried across Anthropic SSE lines.
struct AnthropicLineState<'a> {
    /// Current `event:` label awaiting its `data:` line.
    current_event_type: &'a mut String,
    /// Accumulated usage and tool-call state.
    event_state: &'a mut EventParseState,
}

/// Consume an Anthropic SSE byte stream, dispatching events as they arrive.
///
/// Uses `bytes_stream()` for true per-chunk streaming instead of buffering the
/// full response body. Tracks `current_event_type` across lines - Anthropic
/// sends `event:` before `data:` in each SSE block, so the type is captured
/// when the `event:` line arrives and consumed when `data:` follows.
/// Pending tool-call state persists across chunks for fragmented JSON arguments.
/// Emits `StreamChunk::Usage` before `StreamChunk::Done` once all usage fields
/// are collected from `message_start` and `message_delta` events.
async fn stream_anthropic_events(ctx: RequestContext, response: reqwest::Response) {
    let mut stream = response.bytes_stream();
    let mut buffer = AccumulationBuffer::new();

    while let Some(chunk_result) = stream.next().await {
        let Some(chunk) = read_stream_chunk_or_emit(chunk_result, &ctx).await else {
            return;
        };
        process_stream_lines(&chunk, &ctx, &mut buffer).await;
    }
    if !buffer.carry.as_str().is_empty() {
        process_trailing_line(
            buffer.carry.as_str(),
            &ctx,
            TrailingLineContext {
                current_event_type: &mut buffer.current_event_type,
                state: &mut buffer.state,
            },
        )
        .await;
    }
}

async fn read_stream_chunk_or_emit<T: AsRef<[u8]>>(
    chunk_result: Result<T, reqwest::Error>,
    ctx: &RequestContext,
) -> Option<T> {
    match chunk_result {
        Ok(chunk) => Some(chunk),
        Err(error) => {
            let _ = ctx
                .reply_tx
                .send(StreamChunk::Error(OutputText::from(error.to_string())))
                .await;
            None
        }
    }
}

async fn process_stream_lines<T: AsRef<[u8]>>(
    chunk: &T,
    ctx: &RequestContext,
    buffer: &mut AccumulationBuffer,
) {
    for line in drain_complete_sse_lines(&mut buffer.carry, SseChunk::from(chunk.as_ref())) {
        process_trailing_line(
            &line,
            ctx,
            TrailingLineContext {
                current_event_type: &mut buffer.current_event_type,
                state: &mut buffer.state,
            },
        )
        .await;
    }
}

struct TrailingLineContext<'a> {
    current_event_type: &'a mut String,
    state: &'a mut EventParseState,
}

async fn process_trailing_line(
    line: &str,
    ctx: &RequestContext,
    line_context: TrailingLineContext<'_>,
) {
    let mut line_state = AnthropicLineState {
        current_event_type: line_context.current_event_type,
        event_state: line_context.state,
    };
    process_anthropic_line(line, &mut line_state, ctx).await;
}

/// Build the Anthropic Messages API request body from a `RequestContext`.
///
/// Omits `tools` when the tools list is empty - Anthropic rejects `"tools": []`.
/// When `payload.cache` is `Some`, replaces the plain `"system"` string with a
/// content-block array carrying `cache_control` markers on each tier, enabling
/// Anthropic's prompt caching. `max_tokens` and `temperature` are sourced from
/// `ctx.params` (set from agent config) so the values are always consistent
/// with the runtime configuration.
fn build_anthropic_body(ctx: &RequestContext) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert("model".into(), ctx.endpoint.model.as_str().into());
    body.insert(
        "messages".into(),
        body::to_anthropic_messages(&ctx.payload.messages),
    );
    body.insert("stream".into(), true.into());
    body.insert("max_tokens".into(), ctx.params.max_tokens.inner().into());
    body.insert("temperature".into(), ctx.params.temperature.inner().into());
    let system_text = body::extract_system_text(&ctx.payload.messages);
    match &ctx.payload.cache {
        Some(snapshot) if !snapshot.tiers.is_empty() => {
            body.insert(
                "system".into(),
                body::build_system_blocks(&system_text, snapshot),
            );
        }
        _ if !system_text.as_str().is_empty() => {
            body.insert("system".into(), system_text.as_str().into());
        }
        _ => {}
    }
    if !ctx.payload.tools.is_empty() {
        body.insert("tools".into(), body::to_anthropic_tools(&ctx.payload.tools));
    }
    serde_json::Value::Object(body)
}

/// Dispatch a single Anthropic SSE event to the reply channel.
///
/// Reads event type and data from `event`; updates `state.usage` on
/// `message_start` and `message_delta`; accumulates tool JSON in
/// `state.tool_call` on `content_block_start/delta/stop`; emits `Token` for
/// `text_delta`. On `message_stop`, emits `Usage` then `Done` via `ctx.reply_tx`.
async fn handle_anthropic_event(
    event: &SseEvent<'_>,
    state: &mut EventParseState,
    ctx: &RequestContext,
) {
    let value = parse_event_data(event.data);
    let event_context = ParsedAnthropicEvent {
        event_type: event.event_type,
        value: &value,
    };
    if handle_message_event(event_context, state, ctx).await {
        return;
    }
    let _ = handle_content_block_event(event_context, state, ctx).await;
}

fn parse_event_data(data: &str) -> serde_json::Value {
    serde_json::from_str(data).unwrap_or_else(|_| serde_json::Value::Object(Default::default()))
}

#[derive(Clone, Copy)]
struct ParsedAnthropicEvent<'a> {
    event_type: &'a str,
    value: &'a serde_json::Value,
}

async fn handle_message_event(
    event: ParsedAnthropicEvent<'_>,
    state: &mut EventParseState,
    ctx: &RequestContext,
) -> bool {
    match event.event_type {
        "message_start" => {
            apply_message_start(event.value, state);
            true
        }
        "message_delta" => {
            apply_message_delta(event.value, state);
            true
        }
        "message_stop" => {
            apply_message_stop(state, ctx).await;
            true
        }
        _ => false,
    }
}

async fn handle_content_block_event(
    event: ParsedAnthropicEvent<'_>,
    state: &mut EventParseState,
    ctx: &RequestContext,
) -> bool {
    match event.event_type {
        "content_block_start" => {
            apply_content_block_start(event.value, state);
            true
        }
        "content_block_delta" => {
            apply_content_block_delta(event.value, state, ctx).await;
            true
        }
        "content_block_stop" => {
            apply_content_block_stop(state, ctx).await;
            true
        }
        _ => false,
    }
}

async fn process_anthropic_line(
    line: &str,
    line_state: &mut AnthropicLineState<'_>,
    ctx: &RequestContext,
) {
    if let Some(evt) = line.strip_prefix("event: ") {
        *line_state.current_event_type = evt.trim().to_owned();
    } else if let Some(data) = line.strip_prefix("data: ") {
        let event = SseEvent {
            event_type: line_state.current_event_type.as_str(),
            data: data.trim(),
        };
        handle_anthropic_event(&event, line_state.event_state, ctx).await;
        line_state.current_event_type.clear();
    }
}

fn apply_message_start(value: &serde_json::Value, state: &mut EventParseState) {
    if let Some(model) = value["message"]["model"].as_str() {
        state.usage.model = ModelName::new(model);
    }
    if let Some(cached) = value["message"]["usage"]["cache_read_input_tokens"].as_u64() {
        state.usage.tokens_cached = TokenCount::new(cached);
    }
    if let Some(written) = value["message"]["usage"]["cache_creation_input_tokens"].as_u64() {
        state.usage.cache_write_tokens = TokenCount::new(written);
    }
}

fn apply_content_block_start(value: &serde_json::Value, state: &mut EventParseState) {
    if value["content_block"]["type"] != "tool_use" {
        return;
    }
    let id = value["content_block"]["id"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    state.tool_call.pending_id = Some(ToolCallId::new(id));
    let name = value["content_block"]["name"]
        .as_str()
        .unwrap_or("")
        .to_owned();
    state.tool_call.pending_name = Some(ToolName::new(name));
    state.tool_call.pending_args.clear();
}

async fn apply_content_block_delta(
    value: &serde_json::Value,
    state: &mut EventParseState,
    ctx: &RequestContext,
) {
    match value["delta"]["type"].as_str().unwrap_or("") {
        "text_delta" => {
            let text = value["delta"]["text"].as_str().unwrap_or("");
            let _ = ctx
                .reply_tx
                .send(StreamChunk::Token(OutputText::new(text)))
                .await;
        }
        "input_json_delta" => {
            let partial = value["delta"]["partial_json"].as_str().unwrap_or("");
            state.tool_call.pending_args.push_str(partial);
        }
        _ => {}
    }
}

async fn apply_content_block_stop(state: &mut EventParseState, ctx: &RequestContext) {
    if let Some(name) = state.tool_call.pending_name.take() {
        let id = state
            .tool_call
            .pending_id
            .take()
            .unwrap_or_else(|| ToolCallId::new(""));
        let arguments = serde_json::from_str(&state.tool_call.pending_args)
            .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
        state.tool_call.pending_args.clear();
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

fn apply_message_delta(value: &serde_json::Value, state: &mut EventParseState) {
    if let Some(tokens) = value["usage"]["input_tokens"].as_u64() {
        state.usage.tokens_in = TokenCount::new(tokens);
    }
    if let Some(tokens) = value["usage"]["output_tokens"].as_u64() {
        state.usage.tokens_out = TokenCount::new(tokens);
    }
}

async fn apply_message_stop(state: &EventParseState, ctx: &RequestContext) {
    let llm_usage = build_anthropic_usage(state, ctx);
    let _ = ctx.reply_tx.send(StreamChunk::Usage(llm_usage)).await;
    let _ = ctx.reply_tx.send(StreamChunk::Done).await;
}

fn build_anthropic_usage(state: &EventParseState, ctx: &RequestContext) -> LlmUsage {
    let model_name = if state.usage.model.as_str().is_empty() {
        ModelName::new(ctx.endpoint.model.as_str())
    } else {
        state.usage.model.clone()
    };
    LlmUsage {
        model: OutputText::new(model_name.as_str()),
        token_counts: LlmTokenCounts {
            tokens_in: state.usage.tokens_in,
            tokens_out: state.usage.tokens_out,
            tokens_cached: state.usage.tokens_cached,
            cache_write_tokens: state.usage.cache_write_tokens,
            cost_usd: augur_domain::domain::UsdCost::ZERO,
        },
        temperature: ctx.params.temperature,
    }
}
