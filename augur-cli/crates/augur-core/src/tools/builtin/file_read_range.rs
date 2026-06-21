//! Built-in file_read_range tool: reads a file or a line-number range of a file.
//!
//! Only paths within the configured allowed directories are accessible.
//! Use `file_line_count` first to determine line counts before specifying ranges.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::{FileReadPort, ReadRange};
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;

const TOOL_NAME: &str = "file_read_range";

/// Reads a file or a range of its lines, enforcing allowed-directory access.
///
/// Registered in the tool registry at startup. Delegates I/O and access
/// enforcement to the `FileReadActor` via `FileReadHandle`.
pub struct FileReadRangeTool {
    handle: Box<dyn FileReadPort>,
}

impl FileReadRangeTool {
    /// Create a new tool instance backed by the given file-read provider.
    pub fn new(handle: impl FileReadPort) -> Self {
        FileReadRangeTool {
            handle: Box::new(handle),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileReadRangeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Read a file's contents, optionally limited to a line-number range. \
             Use file_line_count first to discover the file's line count. \
             Omit start_line and end_line to read the full file. \
             Provide only start_line to read from that line to end of file. \
             Provide only end_line to read from the beginning to that line. \
             Provide both to read the inclusive range between them. \
             Line numbers are 1-indexed.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the file to read"
                    },
                    "start_line": {
                        "type": "integer",
                        "description": "First line to include (1-indexed, inclusive). Omit to start from the beginning."
                    },
                    "end_line": {
                        "type": "integer",
                        "description": "Last line to include (1-indexed, inclusive). Omit to read to end of file."
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(tool = "file_read_range"))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let path_str = match args["path"].as_str() {
            Some(s) if !s.is_empty() => s.to_owned(),
            _ => return missing_arg_error("path"),
        };
        let range = parse_range(&args);
        let result = self.handle.read_range(FilePath::new(path_str), range).await;
        ToolCallResult::builder()
            .name(ToolName::new(TOOL_NAME))
            .output(result.output)
            .is_error(result.is_error)
            .build()
    }
}

/// Parse optional `start_line` and `end_line` from args into a `ReadRange`.
///
/// Neither present → `Full`. Only `start_line` → `From`. Only `end_line` → `To`.
/// Both present → `Between`. Values are clamped to valid line bounds by the actor.
fn parse_range(args: &serde_json::Value) -> ReadRange {
    let start = args["start_line"].as_u64().map(|n| n as usize);
    let end = args["end_line"].as_u64().map(|n| n as usize);
    match (start, end) {
        (None, None) => ReadRange::Full,
        (Some(s), None) => ReadRange::From(s),
        (None, Some(e)) => ReadRange::To(e),
        (Some(s), Some(e)) => ReadRange::Between(s, e),
    }
}

/// Returns an error result naming the required argument that was missing or empty.
fn missing_arg_error(arg: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(format!(
            "missing or empty '{}' argument",
            arg
        )))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
        .build()
}
