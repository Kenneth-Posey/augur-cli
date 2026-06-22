//! Built-in file_remove tool: removes a file from the filesystem.
//!
//! Only paths within the configured allowed directories are accessible.
//! Symlink targets are denied. Only regular files can be removed.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "file_remove";

/// Removes a file from the filesystem.
///
/// Only paths within the configured allowed directories are accessible.
pub struct FileRemoveTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileRemoveTool {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        FileRemoveTool {
            allowed_dirs: canonical_dirs,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileRemoveTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Remove a file from the filesystem permanently.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file path to remove"
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let path = match args["path"].as_str() {
            Some(s) if !s.is_empty() => FilePath::new(s),
            _ => {
                return result_msg("missing or empty 'path' argument", true);
            }
        };
        let canonical = match resolve_remove_path(Path::new(path.as_str()), &self.allowed_dirs) {
            Ok(p) => p,
            Err(msg) => return result_msg(msg, true),
        };
        match tokio::fs::remove_file(&canonical).await {
            Ok(()) => result_msg("removed", false),
            Err(e) => {
                let msg = match e.kind() {
                    std::io::ErrorKind::NotFound => "file not found",
                    std::io::ErrorKind::PermissionDenied => "write error: permission denied",
                    _ => "write error: permission denied",
                };
                result_msg(msg, true)
            }
        }
    }
}

fn result_msg(output: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(output.into()))
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(is_error))
        .build()
}

fn resolve_remove_path(path: &Path, allowed_dirs: &[PathBuf]) -> Result<PathBuf, String> {
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
