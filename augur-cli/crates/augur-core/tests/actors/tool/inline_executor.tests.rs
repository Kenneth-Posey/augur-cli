use augur_core::actors::tool::handle::ToolExecutor;
use augur_core::actors::tool::inline_executor::InlineToolExecutor;
use augur_domain::domain::string_newtypes::{StringNewtype, ToolCallId, ToolName};
use augur_domain::domain::types::ToolCall;
use augur_core::tools::registry::ToolRegistry;

#[tokio::test]
async fn inline_executor_handles_unknown_tool() {
    let executor = InlineToolExecutor::new(ToolRegistry::new());
    let call = ToolCall {
        id: ToolCallId::new("inline-call"),
        name: ToolName::new("missing-tool"),
        arguments: serde_json::json!({}),
    };

    let result = executor.execute(call).await.expect("execute should return");
    assert!(result.is_error);
    assert!(result.output.as_str().contains("unknown tool"));
}

#[test]
fn inline_executor_exposes_empty_definitions_for_empty_registry() {
    let executor = InlineToolExecutor::new(ToolRegistry::new());
    assert!(executor.definitions().is_empty());
}
