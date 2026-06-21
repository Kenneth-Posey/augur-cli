//! Built-in file_append tool: appends text to the end of a target file.
//!
//! Only paths within the configured allowed directories are accessible.
//! The parent directory of the target path must exist and must be within
//! `allowed_dirs`; the target file itself need not exist yet.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "file_append";

/// Appends text to the end of a target file.
///
/// Only paths within the configured allowed directories are accessible.
/// Delegates path validation to the allowed-directory whitelist before writing.
pub struct FileAppendTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileAppendTool {
    /// Create a new tool instance that restricts writes to `allowed_dirs`.
    ///
    /// Each entry in `allowed_dirs` is canonicalized at construction time;
    /// entries that cannot be canonicalized are silently skipped.
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        FileAppendTool {
            allowed_dirs: canonical_dirs,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileAppendTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Append text to the end of a target file. Creates the file if it does not exist.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file path"
                    },
                    "content": {
                        "type": "string",
                        "description": "Text content to append"
                    }
                },
                "required": ["path", "content"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let (path, content) = match parse_args(&args) {
            Ok(values) => values,
            Err(result) => return result,
        };
        let canonical = match resolve_write_path(Path::new(path.as_str()), &self.allowed_dirs) {
            Ok(p) => p,
            Err(msg) => return file_append_result(msg, true),
        };
        append_content(&canonical, &content).await
    }
}

fn file_append_result(output: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(output.into()))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(is_error))
        .build()
}

async fn append_content(path: &Path, content: &str) -> ToolCallResult {
    use tokio::io::AsyncWriteExt;
    let mut file = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        Ok(f) => f,
        Err(_) => return file_append_result("write error: permission denied", true),
    };
    if file.write_all(content.as_bytes()).await.is_err() {
        return file_append_result("write error: permission denied", true);
    }
    if file.flush().await.is_err() {
        return file_append_result("write error: permission denied", true);
    }
    // Explicitly close the file handle so content is flushed before the test reads it
    drop(file);
    file_append_result("appended", false)
}

/// Canonicalize the parent directory of `path` and verify it falls within `allowed_dirs`.
///
/// Returns the canonical target path (`canonical_parent/filename`) on success.
/// Returns an opaque `"write error: permission denied"` string on all failures so
/// no internal path details are leaked to the caller.
fn resolve_write_path(path: &Path, allowed_dirs: &[PathBuf]) -> Result<PathBuf, String> {
    let canonical = canonical_target_path(path)?;
    reject_symlink_target(&canonical)?;
    ensure_path_within_allowed_dirs(canonical, allowed_dirs)
}

fn canonical_target_path(path: &Path) -> Result<PathBuf, String> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let filename = path
        .file_name()
        .ok_or_else(|| "write error: permission denied".to_owned())?;
    let canonical_parent =
        std::fs::canonicalize(parent).map_err(|_| "write error: permission denied".to_owned())?;
    Ok(canonical_parent.join(filename))
}

fn reject_symlink_target(path: &Path) -> Result<(), String> {
    if let Ok(meta) = std::fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        return Err("write error: permission denied".to_owned());
    }
    Ok(())
}

fn ensure_path_within_allowed_dirs(
    path: PathBuf,
    allowed_dirs: &[PathBuf],
) -> Result<PathBuf, String> {
    if is_within_allowed_dirs(&path, allowed_dirs).is_some() {
        Ok(path)
    } else {
        Err("write error: permission denied".to_owned())
    }
}

fn parse_args(args: &serde_json::Value) -> Result<(FilePath, String), ToolCallResult> {
    let path = match args["path"].as_str() {
        Some(path) if !path.is_empty() => FilePath::new(path),
        _ => {
            return Err(ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(OutputText::new("missing or empty 'path' argument"))
                .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
                .build());
        }
    };
    let content = match args["content"].as_str() {
        Some(content) => content.to_owned(),
        None => {
            return Err(ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(OutputText::new("missing 'content' argument"))
                .is_error(augur_domain::domain::newtypes::IsPredicate::from(true))
                .build());
        }
    };
    Ok((path, content))
}
