use augur_domain::newtypes::IsPredicate;
use augur_domain::string_newtypes::{OutputText, StringNewtype, ToolCallId, ToolName};
use augur_domain::tools::execution::{normalize_tool_execution_result, tool_result_message};
use augur_domain::tools::handler::ToolCallResult;
use augur_domain::types::{Role, ToolCall};

#[test]
fn normalize_tool_execution_result_sets_error_flag_for_execution_failures() {
    let result = normalize_tool_execution_result(
        ToolName::new("shell_exec"),
        Err(anyhow::anyhow!("No such file or directory (os error 2)")),
    );
    assert!(bool::from(result.is_error));
    assert_eq!(result.name.as_str(), "shell_exec");
    assert!(
        result
            .output
            .as_str()
            .contains("No such file or directory (os error 2)")
    );
}

#[test]
fn normalize_tool_execution_result_redacts_email_addresses() {
    let result = normalize_tool_execution_result(
        ToolName::new("shell_exec"),
        Err(anyhow::anyhow!(
            "author john.smith@example.com could not be processed"
        )),
    );
    assert!(bool::from(result.is_error));
    assert!(
        result.output.as_str().contains("[REDACTED_EMAIL]"),
        "expected email in tool error output to be redacted"
    );
    assert!(
        !result.output.as_str().contains("john.smith@example.com"),
        "expected raw email to be absent from normalized error output"
    );
}

#[test]
fn tool_result_message_preserves_tool_id_name_and_output() {
    let call = ToolCall {
        id: ToolCallId::new("call-1"),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({"command":"pwd"}),
    };
    let result = ToolCallResult::builder()
        .name(ToolName::new("shell_exec"))
        .output(OutputText::new("ok"))
        .is_error(IsPredicate::from(false))
        .build();

    let message = tool_result_message(&call, &result);
    assert_eq!(
        message
            .tool_call_id
            .as_ref()
            .map(|id| id.as_str().to_owned()),
        Some("call-1".to_string())
    );
    assert_eq!(message.role, Role::Tool);
    assert_eq!(message.content.as_str(), "[shell_exec]: ok");
}

#[test]
fn tool_result_message_redacts_email_in_output() {
    let call = ToolCall {
        id: ToolCallId::new("call-2"),
        name: ToolName::new("shell_exec"),
        arguments: serde_json::json!({"command":"git log -1"}),
    };
    let result = ToolCallResult::builder()
        .name(ToolName::new("shell_exec"))
        .output(OutputText::new("Author: Jane <jane.doe@example.com>"))
        .is_error(IsPredicate::from(false))
        .build();

    let message = tool_result_message(&call, &result);
    assert!(
        message.content.as_str().contains("[REDACTED_EMAIL]"),
        "expected redacted marker in tool message content"
    );
    assert!(
        !message.content.as_str().contains("jane.doe@example.com"),
        "expected raw email to be absent from tool message content"
    );
}
