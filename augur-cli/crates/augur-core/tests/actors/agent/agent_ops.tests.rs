use augur_core::actors::agent::agent_ops::{
    AgentOutput, build_extended_system_prompt, merge_chunks_into_result, tool_result_message,
};
use augur_core::actors::tool::tool_ops::ToolCall;
use augur_core::tools::ToolDefinition;
use augur_core::tools::handler::ToolCallResult;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolCallId, ToolName};
use augur_domain::domain::types::Role;

#[test]
fn merge_chunks_no_tool_call() {
    let result = merge_chunks_into_result(&OutputText::new("hello world"), None);
    assert_eq!(result.text, OutputText::new("hello world"));
    assert!(result.tool_call.is_none());
}

#[test]
fn merge_chunks_with_tool_call() {
    let call = ToolCall {
        id: ToolCallId::new("call_test"),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({"cmd": "ls"}),
    };
    let result = merge_chunks_into_result(&OutputText::new("text"), Some(call.clone()));
    let got = result.tool_call.expect("expected Some tool_call");
    assert_eq!(got.name, call.name);
}

#[test]
fn tool_result_message_role_is_tool() {
    let call = ToolCall {
        id: ToolCallId::new("call_abc"),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({}),
    };
    let res = ToolCallResult::builder()
        .name(call.name.clone())
        .output(OutputText::new("output"))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
        .build();
    let msg = tool_result_message(&call, &res);
    assert_eq!(msg.role, Role::Tool);
}

#[test]
fn tool_result_message_content_contains_name_prefix() {
    let call = ToolCall {
        id: ToolCallId::new("call_abc"),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({}),
    };
    let res = ToolCallResult::builder()
        .name(call.name.clone())
        .output(OutputText::new("ran ok"))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
        .build();
    let msg = tool_result_message(&call, &res);
    assert!(msg.content.as_str().contains("shell_exec"));
}

#[test]
fn build_extended_system_prompt_includes_tool_names_and_descriptions() {
    let base = OutputText::new("You are a helpful assistant.");
    let tools = vec![
        ToolDefinition::new("shell_exec", "Run a shell command.", serde_json::json!({})),
        ToolDefinition::new("file_read", "Read a file.", serde_json::json!({})),
    ];
    let result = build_extended_system_prompt(&base, &tools);
    let text = result.as_str();
    assert!(text.contains("You are a helpful assistant."));
    assert!(text.contains("shell_exec"));
    assert!(text.contains("Run a shell command."));
    assert!(text.contains("file_read"));
    assert!(text.contains("Read a file."));
}

#[test]
fn build_extended_system_prompt_no_tools_returns_base() {
    let base = OutputText::new("Base prompt.");
    let result = build_extended_system_prompt(&base, &[] as &[ToolDefinition]);
    assert_eq!(result.as_str(), "Base prompt.");
}

#[test]
fn build_extended_system_prompt_adds_size_check_guidance_when_registered() {
    let base = OutputText::new("Base prompt.");
    let tools = vec![ToolDefinition::new(
        "size_check",
        "Check output size before large operations.",
        serde_json::json!({}),
    )];
    let result = build_extended_system_prompt(&base, &tools);
    assert!(result.as_str().contains("call `size_check` first"));
    assert!(
        result
            .as_str()
            .contains("proceed, filter, paginate, or split")
    );
}

#[test]
fn agent_output_interrupted_variant_exists() {
    let output = AgentOutput::Interrupted;
    assert!(matches!(output, AgentOutput::Interrupted));
}
