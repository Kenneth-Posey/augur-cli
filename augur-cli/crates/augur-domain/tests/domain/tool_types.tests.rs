#![allow(clippy::duplicate_mod)]
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolDescription, ToolName};
use augur_domain::domain::tool_types::{ToolCallResult, ToolDefinition};

#[path = "../support/rustdoc.tests.rs"]
mod rustdoc_support;

/// Verifies ToolDefinition::new stores the provided name, description, and schema unchanged.
#[test]
fn tool_definition_new_populates_all_fields() {
    let parameters = serde_json::json!({
        "type": "object",
        "properties": {
            "command": { "type": "string" }
        },
        "required": ["command"]
    });

    let definition = ToolDefinition::new("shell_exec", "Run a shell command.", parameters.clone());

    assert_eq!(definition.name, ToolName::new("shell_exec"));
    assert_eq!(
        definition.description,
        ToolDescription::new("Run a shell command.")
    );
    assert_eq!(definition.parameters, parameters);
}

/// Verifies ToolDefinition serde round-trips as a public API payload shape.
#[test]
fn tool_definition_serde_roundtrip_preserves_public_fields() {
    let original = ToolDefinition::new(
        "file_read",
        "Read a file from disk.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }),
    );

    let json = serde_json::to_value(&original).unwrap();
    assert_eq!(json["name"], "file_read");
    assert_eq!(json["description"], "Read a file from disk.");
    assert_eq!(json["parameters"]["type"], "object");

    let decoded: ToolDefinition = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.name, original.name);
    assert_eq!(decoded.description, original.description);
    assert_eq!(decoded.parameters, original.parameters);
}

/// Verifies ToolCallResult builder accepts the required fields and leaves session_log empty by default.
#[test]
fn tool_call_result_builder_defaults_session_log_to_none() {
    let result = ToolCallResult::builder()
        .name(ToolName::new("shell_exec"))
        .output(OutputText::new("stdout"))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
        .build();

    assert_eq!(result.name, ToolName::new("shell_exec"));
    assert_eq!(result.output, OutputText::new("stdout"));
    assert!(!result.is_error);
    assert_eq!(result.session_log, None);
}

/// Verifies ToolCallResult can carry an optional session log alongside an error result.
#[test]
fn tool_call_result_builder_preserves_session_log_and_error_flag() {
    let result = ToolCallResult::builder()
        .name(ToolName::new("file_read"))
        .output(OutputText::new("permission denied"))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
        .session_log(OutputText::new("file_read failed"))
        .build();

    assert_eq!(result.name.as_str(), "file_read");
    assert_eq!(result.output.as_str(), "permission denied");
    assert!(result.is_error);
    assert_eq!(
        result.session_log.as_ref().map(|value| value.as_str()),
        Some("file_read failed")
    );
}

/// Verifies ToolDefinition and ToolCallResult expose public rustdoc for the mirrored API surface.
#[test]
fn tool_types_public_api_has_rustdoc_pages() {
    let tool_definition_html =
        rustdoc_support::rustdoc_html("augur_domain/domain/tool_types/struct.ToolDefinition.html");
    assert!(
        tool_definition_html
            .contains("Schema describing a tool available to the LLM for function calling."),
        "expected ToolDefinition rustdoc to contain its public summary",
    );

    let tool_call_result_html =
        rustdoc_support::rustdoc_html("augur_domain/domain/tool_types/struct.ToolCallResult.html");
    assert!(
        tool_call_result_html.contains("The result of executing a tool call."),
        "expected ToolCallResult rustdoc to contain its public summary",
    );
    assert!(
        tool_call_result_html.contains("struct.OutputText.html"),
        "expected ToolCallResult rustdoc to reference OutputText",
    );
}
