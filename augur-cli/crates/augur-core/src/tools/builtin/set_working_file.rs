//! Built-in set_working_file tool: tells the cache actor which file is being edited.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::CacheToolPort;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::PathBuf;

const TOOL_NAME: &str = "set_working_file";

/// Tells the cache actor which source file is currently being worked on.
///
/// Triggers a full dependency analysis and snapshot rebuild from the target's
/// transitive closure. Registered in `wiring.rs::build_registry` when a
/// `CacheHandle` is available.
pub struct SetWorkingFileTool {
    cache: Box<dyn CacheToolPort>,
}

impl SetWorkingFileTool {
    /// Create a new tool bound to the given cache provider.
    ///
    /// Each `execute` call sends a working-file request through the provider.
    pub fn new(cache: impl CacheToolPort) -> Self {
        Self {
            cache: Box::new(cache),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for SetWorkingFileTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Tell the system which file you are currently editing. This triggers a \
             dependency analysis and prepares relevant source files for context.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the source file being edited."
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(tool = "set_working_file"))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let path_str = match args["path"].as_str() {
            Some(s) if !s.is_empty() => s.to_owned(),
            _ => return error_result("missing or empty 'path' argument"),
        };
        let path = PathBuf::from(path_str);
        match self.cache.set_working_file(path).await {
            Ok(()) => ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(OutputText::new(
                    "working file set; dependency analysis started",
                ))
                .is_error(augur_domain::domain::newtypes::IsPredicate::from(false))
                .build(),
            Err(e) => ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(OutputText::new(e.to_string()))
                .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
                .build(),
        }
    }
}

/// Build an error `ToolCallResult` for `set_working_file` with the given message.
fn error_result(msg: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(msg))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
        .build()
}
