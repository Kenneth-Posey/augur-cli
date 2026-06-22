use augur_core::actors::tool::handle::ToolExecutor;
use augur_core::actors::tool::tool_actor::spawn;
use augur_core::actors::tool::tool_ops::ToolCall;
use augur_domain::domain::string_newtypes::{StringNewtype, ToolCallId, ToolName};
use augur_core::tools::registry::ToolRegistry;

#[tokio::test]
async fn handle_exposes_definitions_snapshot() {
    let (_join, handle) = spawn(ToolRegistry::new());
    assert!(handle.definitions().is_empty());
    handle.shutdown();
}

#[tokio::test]
async fn handle_execute_returns_not_found_for_unknown_tool() {
    let (_join, handle) = spawn(ToolRegistry::new());
    let call = ToolCall {
        id: ToolCallId::new("call-1"),
        name: ToolName::new("unknown"),
        arguments: serde_json::json!({}),
    };
    let result = handle.execute(call).await.expect("tool execute should return");
    assert!(result.is_error);
    assert!(result.output.as_str().contains("not found"));
}
