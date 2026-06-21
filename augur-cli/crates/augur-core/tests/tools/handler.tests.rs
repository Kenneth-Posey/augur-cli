use augur_core::tools::handler::{ToolCallResult, ToolHandler};
use augur_domain::domain::newtypes::IsPredicate;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};

/// Verifies the ToolCallResult re-export remains usable from the tools layer.
#[test]
fn tool_call_result_reexport_is_usable() {
    let result = ToolCallResult::builder()
        .name(ToolName::new("echo"))
        .output(OutputText::new("ok"))
        .is_error(IsPredicate::from(false))
        .session_log(OutputText::new("session"))
        .build();

    assert_eq!(result.name, ToolName::new("echo"));
    assert_eq!(result.output, OutputText::new("ok"));
}

/// Verifies the ToolHandler trait bound is available to tests in this module.
#[test]
fn tool_handler_trait_bound_is_addressable() {
    fn _uses_bound<T: ToolHandler>() {}

    assert_eq!(stringify!(ToolHandler), "ToolHandler");
    let _ = _uses_bound::<NoopHandler>;
}

struct NoopHandler;

#[async_trait::async_trait]
impl ToolHandler for NoopHandler {
    fn definition(&self) -> augur_core::tools::ToolDefinition {
        augur_core::tools::ToolDefinition::new(
            "noop",
            "noop handler",
            serde_json::json!({"type": "object"}),
        )
    }

    async fn execute(&self, _args: serde_json::Value) -> ToolCallResult {
        ToolCallResult::builder()
            .name(ToolName::new("noop"))
            .output(OutputText::new("ok"))
            .is_error(IsPredicate::from(false))
            .session_log(OutputText::new("session"))
            .build()
    }
}
