//! Built-in file_line_count tool: returns the number of lines in a file.
//!
//! Call this before `file_read_range` to determine total line count and plan
//! which ranges to request. Only paths within the configured allowed directories
//! are accessible.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::FileReadPort;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;

const TOOL_NAME: &str = "file_line_count";

/// Returns the number of lines in a file, enforcing allowed-directory access.
///
/// Registered in the tool registry at startup. Delegates I/O and access
/// enforcement to the `FileReadActor` via `FileReadHandle`.
pub struct FileLineCountTool {
    handle: Box<dyn FileReadPort>,
}

impl FileLineCountTool {
    /// Create a new tool instance backed by the given file-read provider.
    pub fn new(handle: impl FileReadPort) -> Self {
        FileLineCountTool {
            handle: Box::new(handle),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileLineCountTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Return the number of lines in a file. \
             Use this before file_read_range to discover a file's total line count \
             so you can plan which ranges to request.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the file"
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(tool = "file_line_count"))]
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
        let result = self.handle.line_count(FilePath::new(path_str)).await;
        ToolCallResult::builder()
            .name(ToolName::new(TOOL_NAME))
            .output(result.output)
            .is_error(result.is_error)
            .build()
    }
}
