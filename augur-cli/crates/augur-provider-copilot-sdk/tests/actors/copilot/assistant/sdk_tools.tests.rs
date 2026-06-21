//! Tests for `sdk_tools` assistant module.
//!
//! Validates `query_user_tool_def` schema shape by inspecting the public
//! `parameters_schema` field of the returned `Tool`. All functions are
//! feature-gated; tests run only with `copilot-executor`.

#[cfg(test)]
mod suite {
    /// `query_user_tool_def` returns a tool whose `parameters_schema` lists
    /// `"question"` as a required property. Validates the contract between the
    /// tool definition registered on the SDK session and the Copilot model's
    /// expectation for calling the tool.
    #[test]
    fn query_user_tool_def_has_required_question_field() {
        use augur_provider_copilot_sdk::actors::copilot::assistant::sdk_tools::query_user_tool_def;

        let tool = query_user_tool_def();
        let schema = &tool.parameters_schema;

        let required = &schema["required"];
        let has_question = required
            .as_array()
            .map(|arr: &Vec<serde_json::Value>| arr.iter().any(|v| v.as_str() == Some("question")))
            .unwrap_or(false);

        assert!(
            has_question,
            "'question' must be listed in required fields; schema: {schema:?}"
        );
    }

    /// `query_user_tool_def` schema includes a `"choices"` property of type
    /// `"array"`. Validates the optional choices field is properly described so
    /// the Copilot model can supply it when offering predefined options.
    #[test]
    fn query_user_tool_def_schema_has_choices_array_property() {
        use augur_provider_copilot_sdk::actors::copilot::assistant::sdk_tools::query_user_tool_def;

        let tool = query_user_tool_def();
        let schema = &tool.parameters_schema;

        let choices_type = &schema["properties"]["choices"]["type"];
        assert_eq!(
            choices_type.as_str(),
            Some("array"),
            "expected 'choices' property type to be 'array'; schema: {schema:?}"
        );
    }
}
