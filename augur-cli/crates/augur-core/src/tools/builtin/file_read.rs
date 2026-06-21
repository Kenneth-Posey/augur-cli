//! Built-in file_read tool: reads a file's full contents.
//!
//! Only paths within the configured allowed directories are accessible.
//! Delegates I/O and access enforcement to the `FileReadActor` via `FileReadHandle`.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::{FileReadPort, ReadRange};
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;

const TOOL_NAME: &str = "file_read";

/// Reads the full contents of a file, enforcing allowed-directory access.
///
/// Registered in the tool registry at startup. Delegates I/O and access
/// enforcement to the `FileReadActor` via `FileReadHandle`.
pub struct FileReadTool {
    handle: Box<dyn FileReadPort>,
}

impl FileReadTool {
    /// Create a new tool instance backed by the given file-read provider.
    pub fn new(handle: impl FileReadPort) -> Self {
        FileReadTool {
            handle: Box::new(handle),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Read the full contents of a file.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file path"
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let path_str = match args["path"].as_str() {
            Some(s) if !s.is_empty() => s.to_owned(),
            _ => {
                return ToolCallResult::builder()
                    .name(ToolName::new(TOOL_NAME))
                    .output(OutputText::new("missing or empty 'path' argument"))
                    .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
                    .build();
            }
        };
        let path = FilePath::new(path_str);
        let result = self.handle.read_range(path, ReadRange::Full).await;
        ToolCallResult::builder()
            .name(ToolName::new(TOOL_NAME))
            .output(result.output)
            .is_error(result.is_error)
            .build()
    }
}
