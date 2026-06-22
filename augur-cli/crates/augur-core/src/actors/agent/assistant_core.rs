use super::agent_ops::{AgentOutput, TurnConfig, DEFAULT_MAX_CONTEXT_LENGTH};
use super::history::ConversationHistory;
use crate::actors::cache::handle::CacheHandle;
use augur_domain::domain::newtypes::{Count, IsPredicate, NumericNewtype, TokenCount};
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype};
use augur_domain::domain::task_types::InstructionPrefix;
use augur_domain::domain::{
    CancelSignal, EndpointName, ExecutionSuccess, LlmClient, LlmUsage, Message, Role, StreamChunk,
    ToolCall, ToolExecutor,
};

/// Maximum token estimate for a tool result included in the context window.
///
/// Results exceeding this limit are replaced with a warning asking the LLM to use
/// a more targeted call. Applied to both the conversation history and the OpenRouter
/// context window so that accumulated tool output does not silently grow past
/// provider content-length limits (e.g. Anthropic 1M tokens max).
const TOOL_RESPONSE_CONTEXT_LIMIT_TOKENS: TokenCount = TokenCount::of(50_000);

/// Fraction of `max_context_length` used as the total request-size guard threshold
/// when `request_cap_threshold` is not set.
///
/// The guard warns when estimated request tokens exceed
/// `max_context_length * CAP_FRACTION`. The remaining headroom (20%) accounts
/// for system prompt, tool definitions, and serialization overhead.
const CAP_FRACTION_NUMERATOR: u64 = 80;
const CAP_FRACTION_DENOMINATOR: u64 = 100;

/// Compute the total request-size cap for the LLM provider given the selected
/// model's max context length.
///
/// Returns `max_context_length * 80 / 100` when `max_context_length > 0`,
/// falling back to the default max context length scaled by the same fraction.
fn request_cap_for_context(max_context_length: TokenCount) -> TokenCount {
    let base = if max_context_length > TokenCount::ZERO {
        max_context_length
    } else {
        DEFAULT_MAX_CONTEXT_LENGTH
    };
    TokenCount::new(base.inner() * CAP_FRACTION_NUMERATOR / CAP_FRACTION_DENOMINATOR)
}

/// Compute the effective request-size cap, preferring `request_cap_threshold`
/// from the model's provider catalog (e.g. `auto_compact_threshold`) over the
/// fraction-based calculation from `max_context_length`.
fn effective_request_cap(cfg: &TurnConfig) -> TokenCount {
    if cfg.request_cap_threshold > TokenCount::ZERO {
        cfg.request_cap_threshold
    } else {
        request_cap_for_context(cfg.max_context_length)
    }
}

/// Estimate token count for a string using word and character heuristics.
///
/// Uses `max(word_count, char_count / 2)` as a conservative over-estimate so
/// that we err on the side of capping rather than passing oversized payloads.
fn estimate_output_tokens(text: &impl StringNewtype) -> TokenCount {
    let s = text.as_str();
    let by_words = s.split_whitespace().count();
    let by_chars = (s.len().saturating_add(1)) / 2;
    TokenCount::new(by_words.max(by_chars).max(1) as u64)
}

/// Estimate the total tokens across all request messages by summing per-message
/// content estimates. Uses the same heuristic as `estimate_output_tokens` for each
/// message's content and a flat overhead per message for role/timestamp metadata.
fn estimate_messages_tokens(messages: &[Message]) -> TokenCount {
    const OVERHEAD_PER_MESSAGE: u64 = 8;
    let total: u64 = messages
        .iter()
        .map(|msg| {
            let content_est = estimate_output_tokens(&msg.content);
            content_est.inner().saturating_add(OVERHEAD_PER_MESSAGE)
        })
        .sum();
    TokenCount::new(total)
}

/// Build the message pushed into the OpenRouter context window for a tool result.
///
/// If the output is within the token budget, the full result is returned. Otherwise
/// a warning is returned asking the LLM to issue a more targeted request. The
/// full output is only persisted to conversation history when it is within
/// the token budget; oversized results are stored only as a sizing warning
/// to avoid inflating session file sizes.
fn capped_tool_result_message(
    call: &ToolCall,
    result: &augur_domain::domain::ToolCallResult,
) -> Message {
    let estimated = estimate_output_tokens(&result.output);
    if estimated <= TOOL_RESPONSE_CONTEXT_LIMIT_TOKENS {
        return crate::tools::execution::tool_result_message(call, result);
    }
    let warning = OutputText::new(format!(
        "[Output too large (~{} tokens). Please retry with a more targeted request \
         (e.g. specific line ranges, grep patterns, or pagination flags) to reduce \
         output size.]",
        estimated.inner()
    ));
    Message::tool_result(call.id.clone(), &call.name, warning)
}
use std::fmt;
use tokio::sync::{broadcast, mpsc, watch};

#[derive(Clone, Copy)]
/// Optional turn-level runtime extensions for assistant turn execution.
pub struct TurnExtensions<'a> {
    pub cache: Option<&'a CacheHandle>,
    pub instruction_prefix: Option<&'a InstructionPrefix>,
}

#[derive(bon::Builder)]
/// Immutable inputs required to process one assistant turn.
pub struct TurnContext<'a, L, T> {
    pub llm: &'a L,
    pub tools: &'a T,
    pub output_tx: &'a broadcast::Sender<AgentOutput>,
    pub cancel_rx: &'a mut watch::Receiver<CancelSignal>,
    pub ext: TurnExtensions<'a>,
}

/// Result values emitted after processing one assistant turn.
pub struct TurnResult {
    pub usage: Option<LlmUsage>,
    pub error: Option<OutputText>,
    pub messages_len: Count,
}

#[derive(Debug)]
struct Interrupted;

impl fmt::Display for Interrupted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "turn interrupted")
    }
}

impl std::error::Error for Interrupted {}

/// Process one assistant turn against the provided conversation history.
///
/// This is a provider-agnostic turn processor that:
/// 1. Calls the LLM through the generic LlmClient trait
/// 2. Streams and consumes the completion
/// 3. Executes any tool calls in a loop
/// 4. Tracks usage and handles errors
///
/// Provider-specific logic (compaction, auto-retry, message formatting) is
/// handled by the provider-specific LLM client implementation before reaching this code.
pub async fn process_turn<L: LlmClient, T: ToolExecutor>(
    history: &mut ConversationHistory,
    ctx: TurnContext<'_, L, T>,
    cfg: TurnConfig,
) -> TurnResult {
    use augur_domain::domain::traits::CompletionRequest;

    let TurnContext {
        llm,
        tools,
        output_tx,
        cancel_rx,
        ext,
    } = ctx;

    let mut last_usage: Option<LlmUsage> = None;
    let mut iterations = Count::ZERO;
    let max_iterations = cfg.max_iterations;
    let mut previous_iteration_had_tool_call = false;
    let mut empty_post_tool_retry_budget: u8 = 0;
    loop {
        // Check iteration limit
        if iterations >= max_iterations {
            let _ = output_tx.send(AgentOutput::Error(OutputText::new(format!(
                "[Turn limit reached ({} iterations)]",
                max_iterations.inner()
            ))));
            return TurnResult {
                usage: last_usage,
                error: Some(OutputText::new("Maximum iterations reached")),
                messages_len: history.len(),
            };
        }
        iterations += Count::new(1);

        // Build completion request
        let tool_definitions = tools.definitions().to_vec();
        let raw_messages = if is_openrouter_endpoint(&cfg.endpoint, &cfg.app_config).0 {
            history.openrouter_context_messages_for_request()
        } else {
            history.messages_for_request()
        };
        let request_messages = inject_prefix_if_openrouter(
            &cfg.endpoint,
            raw_messages,
            ext.instruction_prefix,
            &cfg.app_config,
        );

        // Guard: total estimated tokens across all request messages must not
        // exceed the effective request cap (model's auto_compact_threshold, or
        // 80% of max_context_length). When the limit is exceeded, warn via a
        // system message and complete the turn gracefully (no error) so the
        // agentic loop continues and the user can use /compact or /new-session.
        {
            let cap = effective_request_cap(&cfg);
            let estimated_total = estimate_messages_tokens(&request_messages);
            if estimated_total > cap {
                let msg = format!(
                    "[Request too large: ~{} estimated tokens. \
                     The total context exceeds the safe limit of {}.\n\
                     Use `/new-session` to start a fresh conversation \
                     or `/compact` to compress the current context.]",
                    estimated_total.inner(),
                    cap.inner(),
                );
                tracing::warn!(
                    event = "request_too_large_warning",
                    estimated_tokens = estimated_total.inner(),
                    cap_tokens = cap.inner(),
                    action = "warn_and_complete",
                );
                let _ = output_tx.send(AgentOutput::SystemMessage(OutputText::new(msg.clone())));
                return TurnResult {
                    usage: last_usage,
                    error: None,
                    messages_len: history.len(),
                };
            }
        }
        let role_counts =
            request_messages
                .iter()
                .fold((0usize, 0usize, 0usize, 0usize), |acc, msg| {
                    match msg.role {
                        Role::System => (acc.0 + 1, acc.1, acc.2, acc.3),
                        Role::User => (acc.0, acc.1 + 1, acc.2, acc.3),
                        Role::Assistant => (acc.0, acc.1, acc.2 + 1, acc.3),
                        Role::Tool => (acc.0, acc.1, acc.2, acc.3 + 1),
                    }
                });
        let assistant_tool_call_messages = request_messages
            .iter()
            .filter(|msg| msg.role == Role::Assistant && msg.tool_calls.is_some())
            .count();
        tracing::debug!(
            event = "llm_request_meta",
            endpoint = cfg.endpoint.as_str(),
            iteration = iterations.inner(),
            messages_count = request_messages.len(),
            tools_count = tool_definitions.len(),
            system_messages = role_counts.0,
            user_messages = role_counts.1,
            assistant_messages = role_counts.2,
            tool_messages = role_counts.3,
            assistant_tool_call_messages,
        );
        let cache_snapshot = match ext.cache {
            Some(handle) => handle.get_snapshot().await.unwrap_or(None),
            None => None,
        };
        let request = CompletionRequest::builder()
            .endpoint(cfg.endpoint.clone())
            .messages(request_messages)
            .tools(tool_definitions)
            .maybe_cache(cache_snapshot)
            .maybe_model_override(cfg.model_override.clone())
            .build();

        // Request completion from LLM
        let stream_rx = llm.complete_stream(request);

        // Consume the stream
        let (text_buf, mut tool_call, usage) =
            match consume_stream(stream_rx, output_tx, cancel_rx).await {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!(
                        event = "turn_stream_error",
                        endpoint = cfg.endpoint.as_str(),
                        iteration = iterations.inner(),
                        error = %e,
                    );
                    let error_text = OutputText::new(e.to_string());
                    let _ = output_tx.send(AgentOutput::Error(error_text.clone()));
                    return TurnResult {
                        usage: last_usage,
                        error: Some(error_text),
                        messages_len: history.len(),
                    };
                }
            };

        let usage_seen = usage.is_some();
        if let Some(u) = usage {
            last_usage = Some(u);
        }
        tracing::debug!(
            event = "turn_stream_summary",
            endpoint = cfg.endpoint.as_str(),
            iteration = iterations.inner(),
            text_chars = text_buf.len(),
            tool_call_seen = tool_call.is_some(),
            usage_seen,
        );

        // If we got a tool call, execute it and loop
        if let Some(call) = tool_call.take() {
            tracing::debug!(
                event = "tool_call_received",
                endpoint = cfg.endpoint.as_str(),
                tool_name = call.name.as_str(),
                tool_id_empty = call.id.as_str().is_empty(),
                arguments_kind = tool_arguments_kind(&call.arguments),
                arguments_serialized_len = tool_arguments_len(&call.arguments),
                assistant_text_chars = text_buf.len(),
            );
            let _ = output_tx.send(AgentOutput::ToolCallStarted {
                name: call.name.clone(),
                args: call.arguments.clone(),
            });
            let result = crate::tools::execution::normalize_tool_execution_result(
                call.name.clone(),
                tools.execute(call.clone()).await,
            );
            previous_iteration_had_tool_call = true;
            // Budget covers transient 0-token API responses (rate-limit glitches).
            // 5 retries for error results, 8 for successful results.
            empty_post_tool_retry_budget = if result.is_error.0 { 5 } else { 8 };
            let _ = output_tx.send(AgentOutput::ToolCallCompleted {
                name: result.name.clone(),
                success: ExecutionSuccess::from(!result.is_error.0),
                result: Some(result.output.clone()),
                session_log: result.session_log.clone(),
            });
            if !text_buf.is_empty() {
                let _ = output_tx.send(AgentOutput::MessageBreak);
            }
            tracing::debug!(
                event = "tool_execution_result",
                endpoint = cfg.endpoint.as_str(),
                tool_name = call.name.as_str(),
                is_error = result.is_error.0,
                output_chars = result.output.as_str().len(),
                next_action = "continue_llm",
            );
            history.push(Message::assistant_with_tool_calls(
                OutputText::new(text_buf.clone()),
                vec![call.clone()],
            ));
            let conversation_msg = capped_tool_result_message(&call, &result);
            history.push_conversation(conversation_msg);
            history.push_openrouter_context(capped_tool_result_message(&call, &result));
            // Continue loop to call LLM again with tool result
            continue;
        }

        if text_buf.is_empty() && previous_iteration_had_tool_call {
            if empty_post_tool_retry_budget > 0 {
                empty_post_tool_retry_budget -= 1;
                tracing::warn!(
                    event = "empty_post_tool_follow_up_retry",
                    endpoint = cfg.endpoint.as_str(),
                    iteration = iterations.inner(),
                    retries_remaining = empty_post_tool_retry_budget,
                );
                continue;
            }
            let error_text = OutputText::new(
                "No response after repeated retries - the LLM returned empty output. Please try again.".to_string(),
            );
            tracing::warn!(
                event = "empty_post_tool_follow_up_give_up",
                endpoint = cfg.endpoint.as_str(),
                iteration = iterations.inner(),
                action = "return_error",
            );
            let _ = output_tx.send(AgentOutput::Error(error_text.clone()));
            return TurnResult {
                usage: last_usage,
                error: Some(error_text),
                messages_len: history.len(),
            };
        }

        // No tool call or assistant text only - we're done
        if !text_buf.is_empty() {
            history.push(Message::assistant(OutputText::new(text_buf)));
        }
        tracing::debug!(
            event = "turn_decision",
            endpoint = cfg.endpoint.as_str(),
            iteration = iterations.inner(),
            decision = "completed_without_tool",
            messages_len = history.len().inner(),
        );
        break;
    }

    TurnResult {
        usage: last_usage,
        error: None,
        messages_len: history.len(),
    }
}

/// Consume completion stream and accumulate tokens/tool calls.
async fn consume_stream(
    mut rx: mpsc::Receiver<StreamChunk>,
    output_tx: &broadcast::Sender<AgentOutput>,
    cancel_rx: &mut watch::Receiver<CancelSignal>,
) -> Result<(String, Option<ToolCall>, Option<LlmUsage>), String> {
    let mut text_buf = String::new();
    let mut tool_call: Option<ToolCall> = None;
    let mut usage: Option<LlmUsage> = None;
    let mut seen_done = false;
    let mut end_reason = "channel_closed";

    loop {
        tokio::select! {
            biased;
            chunk = rx.recv() => {
                match chunk {
                    None => break,
                    Some(StreamChunk::Done) => {
                        seen_done = true;
                        end_reason = "done_chunk";
                        break;
                    }
                    Some(StreamChunk::Error(e)) => return Err(e.to_string()),
                    Some(StreamChunk::Usage(u)) => usage = Some(u),
                    Some(StreamChunk::Token(token)) => {
                        let _ = output_tx.send(AgentOutput::Token(token.clone()));
                        text_buf.push_str(token.as_str());
                    }
                    Some(StreamChunk::ToolCall { id, name, arguments }) => {
                        if tool_call.is_none() {
                            tracing::debug!(
                                event = "consumer_tool_call_chunk",
                                tool_name = name.as_str(),
                                tool_id_empty = id.as_str().is_empty(),
                                arguments_kind = tool_arguments_kind(&arguments),
                                arguments_serialized_len = tool_arguments_len(&arguments),
                            );
                            tool_call = Some(ToolCall { id, name, arguments });
                        } else {
                            tracing::debug!(
                                event = "consumer_additional_tool_call_ignored",
                                tool_name = name.as_str(),
                            );
                        }
                    }
                    Some(StreamChunk::RateLimitRetry(wait_secs)) => {
                        let notice = format!("[rate limit - waiting {}s...]\n", wait_secs);
                        let _ = output_tx.send(AgentOutput::Token(OutputText::new(notice)));
                        let _ = output_tx.send(AgentOutput::BackoffStarted(wait_secs));
                    }
                }
            }
            _ = cancel_rx.changed() => {
                if matches!(*cancel_rx.borrow(), CancelSignal::Cancelled) {
                    return Err("turn interrupted".to_string());
                }
            }
        }
    }

    tracing::debug!(
        event = "consumer_stream_end",
        end_reason,
        seen_done,
        text_chars = text_buf.len(),
        tool_call_seen = tool_call.is_some(),
        usage_seen = usage.is_some(),
    );

    if !seen_done && text_buf.is_empty() && tool_call.is_none() {
        return Err("no response received - stream disconnected before completion".to_string());
    }

    Ok((text_buf, tool_call, usage))
}

fn tool_arguments_kind(arguments: &serde_json::Value) -> &'static str {
    match arguments {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn tool_arguments_len(arguments: &serde_json::Value) -> usize {
    serde_json::to_string(arguments)
        .map(|s| s.len())
        .unwrap_or(0)
}

/// Return whether the endpoint uses OpenRouter routing semantics.
pub(super) fn is_openrouter_endpoint(
    endpoint: &EndpointName,
    app_config: &augur_domain::config::types::AppConfig,
) -> IsPredicate {
    let provider = app_config
        .endpoints
        .iter()
        .find(|ep| &ep.name == endpoint)
        .map(|ep| &ep.provider);

    if let Some(provider) = provider {
        IsPredicate::from(matches!(
            provider,
            augur_domain::config::types::Provider::OpenRouter
        ))
    } else {
        IsPredicate::from(endpoint.as_str().contains("openrouter"))
    }
}

/// Prepend the instruction prefix only for OpenRouter endpoints.
pub(super) fn inject_prefix_if_openrouter(
    endpoint: &EndpointName,
    messages: Vec<Message>,
    prefix: Option<&InstructionPrefix>,
    app_config: &augur_domain::config::types::AppConfig,
) -> Vec<Message> {
    if !is_openrouter_endpoint(endpoint, app_config).0 {
        return messages;
    }
    match prefix {
        None => messages,
        Some(p) => {
            let mut combined = p.0.clone();
            combined.extend(messages);
            combined
        }
    }
}
