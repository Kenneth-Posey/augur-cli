use crate::domain::string_newtypes::ToolName;
use crate::tools::definition::ToolDefinition;
use crate::tools::handler::ToolHandler;

pub struct ToolRegistry {
    handlers: Vec<Box<dyn ToolHandler>>,
    definitions: Vec<ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: vec![],
            definitions: vec![],
        }
    }

    pub fn register(&mut self, handler: impl ToolHandler + 'static) {
        self.definitions.push(handler.definition());
        self.handlers.push(Box::new(handler));
    }

    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    pub fn find(&self, name: &ToolName) -> Option<&dyn ToolHandler> {
        self.definitions
            .iter()
            .position(|definition| &definition.name == name)
            .and_then(|index| self.handlers.get(index))
            .map(|handler| handler.as_ref())
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
