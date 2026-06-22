use crate::domain::newtypes::IsPredicate;
use crate::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use crate::domain::tool_types::ToolCallResult;
use crate::domain::types::{Message, ToolCall};

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
pub fn tool_result_message(call: &ToolCall, result: &ToolCallResult) -> Message {
    Message::tool_result(
        call.id.clone(),
        &call.name,
        OutputText::new(redact_email_addresses(result.output.as_str())),
    )
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
