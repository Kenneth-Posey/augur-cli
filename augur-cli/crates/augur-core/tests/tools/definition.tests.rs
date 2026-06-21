use augur_core::tools::ToolDefinition;
use augur_domain::domain::string_newtypes::{StringNewtype, ToolDescription, ToolName};

/// Verifies that ToolDefinition::new stores name, description, and parameters correctly.
#[test]
fn tool_definition_new_stores_fields() {
    let params = serde_json::json!({"type":"object","properties":{},"required":[]});
    let def = ToolDefinition::new("my_tool", "does stuff", params.clone());
    assert_eq!(def.name, ToolName::new("my_tool"));
    assert_eq!(def.description, ToolDescription::new("does stuff"));
    assert_eq!(def.parameters, params);
}

/// Verifies that empty tool names and descriptions are accepted without panicking.
#[test]
fn tool_definition_new_allows_empty_name_and_description() {
    let def = ToolDefinition::new("", "", serde_json::json!({}));
    assert_eq!(def.name.as_str(), "");
    assert_eq!(def.description.as_str(), "");
}
