use crate::tools::definition::ToolDefinition;

pub use crate::domain::tool_types::ToolCallResult;

#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync + 'static {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult;
}
