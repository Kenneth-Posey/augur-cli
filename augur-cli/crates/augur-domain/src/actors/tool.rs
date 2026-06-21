use crate::domain::newtypes::IsPredicate;
use crate::domain::string_newtypes::{OutputText, StringNewtype};
use crate::domain::traits::ToolExecutor;
use crate::domain::types::ToolCall;
use crate::tools::definition::ToolDefinition;
use crate::tools::handler::ToolCallResult;
use crate::tools::registry::ToolRegistry;
use std::sync::Arc;

#[derive(Clone)]
pub struct InlineToolExecutor {
    registry: Arc<ToolRegistry>,
    definitions: Arc<Vec<ToolDefinition>>,
}

impl InlineToolExecutor {
    pub fn new(registry: ToolRegistry) -> Self {
        let defs = Arc::new(registry.definitions().to_vec());
        Self {
            registry: Arc::new(registry),
            definitions: defs,
        }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for InlineToolExecutor {
    fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    async fn execute(&self, call: ToolCall) -> anyhow::Result<ToolCallResult> {
        match self.registry.find(&call.name) {
            Some(handler) => Ok(handler.execute(call.arguments).await),
            None => Ok(ToolCallResult::builder()
                .name(call.name.clone())
                .output(OutputText::new(format!("unknown tool: {}", call.name)))
                .is_error(IsPredicate::from(true))
                .build()),
        }
    }
}
