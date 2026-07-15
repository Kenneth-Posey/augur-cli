use crate::domain::newtypes::{IsPredicate, NumericNewtype, TokenCount};
use crate::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use crate::domain::tool_types::ToolCallResult;
use crate::domain::types::{Message, ToolCall};

/// Maximum token estimate for a tool result included in the context window.
///
/// Results exceeding this limit are replaced with a warning asking the LLM to use
/// a more targeted call. Applied to both the conversation history and the OpenRouter
/// context window so that accumulated tool output does not silently grow past
/// provider content-length limits (e.g. Anthropic 1M tokens max).
///
/// This is the default value used when no per-model configuration is available.
/// Callers may override by passing a `cap` argument to [`capped_tool_result_message`].
pub const TOOL_RESPONSE_CONTEXT_LIMIT_TOKENS: TokenCount = TokenCount::of(50_000);

/// Estimate token count for a string using word and character heuristics.
///
/// Uses `max(word_count, char_count / 2)` as a conservative over-estimate so
/// that we err on the side of capping rather than passing oversized payloads.
pub fn estimate_output_tokens(text: &impl StringNewtype) -> TokenCount {
    let s = text.as_str();
    let by_words = s.split_whitespace().count();
    let by_chars = (s.len().saturating_add(1)) / 2;
    TokenCount::new(by_words.max(by_chars).max(1) as u64)
}

/// Normalize a tool execution result for loop continuation.
///
/// Converts transport/execution failures into a `ToolCallResult` with
/// `is_error=true`, preserving the called tool name and error text so callers can
/// append a tool-result message and continue the turn loop.
pub fn normalize_tool_execution_result(
    tool_name: ToolName,
    executed: anyhow::Result<ToolCallResult>,
) -> ToolCallResult {
    match executed {
        Ok(result) => result,
        Err(error) => ToolCallResult::builder()
            .name(tool_name)
            .output(OutputText::new(redact_email_addresses(&error.to_string())))
            .is_error(IsPredicate::from(true))
            .build(),
    }
}

/// Build a conversation tool-result message from a tool call and normalized result.
///
/// The full output is included without size capping. Callers that want to limit
/// tool result size to avoid inflating the context window should use
/// [`capped_tool_result_message`] instead.
pub fn tool_result_message(call: &ToolCall, result: &ToolCallResult) -> Message {
    Message::tool_result(
        call.id.clone(),
        &call.name,
        OutputText::new(redact_email_addresses(result.output.as_str())),
    )
}

/// Build the message pushed into the context window for a tool result, capping
/// oversized outputs to a warning message.
///
/// If the output is within the token budget, the full result is returned. Otherwise
/// a warning is returned asking the LLM to issue a more targeted request. The
/// full output is only persisted to conversation history when it is within
/// the token budget; oversized results are stored only as a sizing warning
/// to avoid inflating session file sizes.
///
/// Use this in tool-calling loops to prevent unbounded context growth from
/// large `shell_exec`, `file_read`, or other tool outputs.
///
/// When `cap` is `None`, the default [`TOOL_RESPONSE_CONTEXT_LIMIT_TOKENS`] is used.
/// When `cap` is `Some`, the caller-provided value overrides the default.
pub fn capped_tool_result_message(
    call: &ToolCall,
    result: &ToolCallResult,
    cap: Option<TokenCount>,
) -> Message {
    let limit = cap.unwrap_or(TOOL_RESPONSE_CONTEXT_LIMIT_TOKENS);
    let estimated = estimate_output_tokens(&result.output);
    if estimated <= limit {
        return tool_result_message(call, result);
    }
    let warning = OutputText::new(format!(
        "[Output too large (~{} tokens). Please retry with a more targeted request \
         (e.g. specific line ranges, grep patterns, or pagination flags) to reduce \
         output size.]",
        estimated.inner()
    ));
    Message::tool_result(call.id.clone(), &call.name, warning)
}

fn redact_email_addresses(input: &str) -> String {
    let mut out = String::new();
    for token in input.split_inclusive(char::is_whitespace) {
        let trimmed = token.trim_end_matches(char::is_whitespace);
        let suffix = &token[trimmed.len()..];
        if looks_like_email(trimmed) {
            out.push_str("[REDACTED_EMAIL]");
        } else {
            out.push_str(trimmed);
        }
        out.push_str(suffix);
    }
    out
}

fn looks_like_email(token: &str) -> bool {
    let start = token
        .char_indices()
        .find(|(_, c)| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    let end = token
        .char_indices()
        .rfind(|(_, c)| c.is_ascii_alphanumeric())
        .map(|(idx, c)| idx + c.len_utf8())
        .unwrap_or(token.len());
    if start >= end {
        return false;
    }
    let core = &token[start..end];
    let mut parts = core.split('@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    if parts.next().is_some() || local.is_empty() || domain.is_empty() {
        return false;
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-'))
    {
        return false;
    }
    if !domain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
    {
        return false;
    }
    domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_tool_result_message_returns_full_output_when_within_budget() {
        let call = ToolCall {
            id: crate::domain::string_newtypes::ToolCallId::new("call_1"),
            name: ToolName::new("file_read"),
            arguments: serde_json::json!({"path": "test.txt"}),
        };
        let small_output = "x".repeat(100);
        let result = ToolCallResult::builder()
            .name(ToolName::new("file_read"))
            .output(OutputText::new(small_output.clone()))
            .is_error(IsPredicate::from(false))
            .build();
        let msg = capped_tool_result_message(&call, &result, None);
        assert!(
            msg.content.as_str().contains(&small_output),
            "expected full output within budget: {}",
            msg.content.as_str()
        );
        assert!(
            !msg.content.as_str().contains("Output too large"),
            "expected no truncation warning"
        );
    }

    #[test]
    fn capped_tool_result_message_truncates_oversized_output() {
        let call = ToolCall {
            id: crate::domain::string_newtypes::ToolCallId::new("call_2"),
            name: ToolName::new("shell_exec"),
            arguments: serde_json::json!({"command": "cat large_file.txt"}),
        };
        // Generate output well over 50K tokens (use ~200K chars = ~100K tokens)
        let large_output = "word ".repeat(60_000);
        let result = ToolCallResult::builder()
            .name(ToolName::new("shell_exec"))
            .output(OutputText::new(large_output))
            .is_error(IsPredicate::from(false))
            .build();
        let msg = capped_tool_result_message(&call, &result, None);
        assert!(
            msg.content.as_str().contains("Output too large"),
            "expected truncation warning in: {}",
            msg.content.as_str()
        );
        assert!(
            !msg.content.as_str().contains("word"),
            "expected no original output after truncation"
        );
    }

    #[test]
    fn capped_tool_result_message_truncates_at_boundary() {
        let call = ToolCall {
            id: crate::domain::string_newtypes::ToolCallId::new("call_3"),
            name: ToolName::new("file_read"),
            arguments: serde_json::json!({"path": "moderate.txt"}),
        };
        // ~55K words = ~55K tokens, just over the 50K limit
        let moderate_output = "token ".repeat(55_000);
        let result = ToolCallResult::builder()
            .name(ToolName::new("file_read"))
            .output(OutputText::new(moderate_output))
            .is_error(IsPredicate::from(false))
            .build();
        let msg = capped_tool_result_message(&call, &result, None);
        assert!(
            msg.content.as_str().contains("Output too large"),
            "expected truncation for over-budget output"
        );
    }

    #[test]
    fn capped_tool_result_message_uses_custom_cap() {
        let call = ToolCall {
            id: crate::domain::string_newtypes::ToolCallId::new("call_4"),
            name: ToolName::new("shell_exec"),
            arguments: serde_json::json!({"command": "ls"}),
        };
        // ~500 tokens, fine for 50K default but over the custom cap of 100
        let output = "word ".repeat(500);
        let result = ToolCallResult::builder()
            .name(ToolName::new("shell_exec"))
            .output(OutputText::new(output))
            .is_error(IsPredicate::from(false))
            .build();
        let msg = capped_tool_result_message(&call, &result, Some(TokenCount::of(100)));
        assert!(
            msg.content.as_str().contains("Output too large"),
            "expected truncation for over-budget output with custom cap"
        );
    }
}
