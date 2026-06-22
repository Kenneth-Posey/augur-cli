use augur_core::actors::tool::tool_ops::build_tool_call;
use augur_domain::domain::string_newtypes::{StringNewtype, ToolCallId, ToolName};
use augur_domain::domain::types::StreamChunk;

/// Verifies that build_tool_call extracts ToolCall from StreamChunk::ToolCall.
#[test]
fn build_tool_call_extracts_tool_call() {
    let chunk = StreamChunk::ToolCall {
        id: ToolCallId::new("call_abc"),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({"command": "ls"}),
    };
    let result = build_tool_call(chunk);
    assert!(result.is_some());
    let call = result.unwrap();
    assert_eq!(call.name, ToolName::new("shell_exec"));
    assert_eq!(call.arguments["command"], "ls");
    assert_eq!(call.id, ToolCallId::new("call_abc"));
}

/// Verifies that build_tool_call returns None for non-ToolCall variants.
#[test]
fn build_tool_call_returns_none_for_non_tool_call() {
    let chunk = StreamChunk::Done;
    assert!(build_tool_call(chunk).is_none());
}
