//! Built-in file_create tool: writes text content to a new file.
//!
//! Only paths within the configured allowed directories are accessible.
//! The parent directory of the target path must exist and must be within
//! `allowed_dirs`; the target file itself must NOT already exist. If the
//! file already exists, the tool warns the LLM and suggests using
//! file_replace, file_insert, file_slice, or file_append instead.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "file_create";

/// Writes text content to a new file. Refuses to overwrite existing files.
///
/// Only paths within the configured allowed directories are accessible.
pub struct FileCreateTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileCreateTool {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        FileCreateTool {
            allowed_dirs: canonical_dirs,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileCreateTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Write text content to a new file. Refuses to overwrite existing files. \
             If the file already exists, use file_replace, file_insert, file_slice, \
             or file_append to modify it instead. Use file_remove to delete a file.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file path"
                    },
                    "content": {
                        "type": "string",
                        "description": "Text content to write"
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
        let canonical = match resolve_create_path(Path::new(path.as_str()), &self.allowed_dirs) {
            Ok(p) => p,
            Err(msg) => return result_msg(msg, true),
        };

        // Refuse to overwrite existing files - warn the LLM
        if canonical.exists() {
            return result_msg(
                "file already exists; use file_replace, file_insert, file_slice, \
                 or file_append to modify it instead",
                false,
            );
        }

        write_content(&canonical, &content).await
    }
}

fn result_msg(output: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(output.into()))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(is_error))
        .build()
}

async fn write_content(path: &Path, content: &str) -> ToolCallResult {
    match tokio::fs::write(path, content.as_bytes()).await {
        Ok(()) => result_msg("written", false),
        Err(_) => result_msg("write error: permission denied", true),
    }
}

/// Canonicalize the parent directory of `path` and verify it falls within `allowed_dirs`.
fn resolve_create_path(path: &Path, allowed_dirs: &[PathBuf]) -> Result<PathBuf, String> {
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
            return Err(result_msg("missing or empty 'path' argument", true));
        }
    };
    let content = match args["content"].as_str() {
        Some(content) => content.to_owned(),
        None => {
            return Err(result_msg("missing 'content' argument", true));
        }
    };
    Ok((path, content))
}
