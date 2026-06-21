//! Built-in file_insert tool: inserts text before or after a unique text anchor.
//!
//! Only paths within the configured allowed directories are accessible.
//! Validates that the anchor text is unique in the file before proceeding.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "file_insert";

/// Inserts text before or after a unique text anchor in a file.
pub struct FileInsertTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileInsertTool {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        FileInsertTool {
            allowed_dirs: canonical_dirs,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileInsertTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Insert text before or after a unique text anchor in a file. \
             The anchor_text must be unique in the file. Use position 'before' \
             to insert before the anchor, or 'after' to insert after it.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file path"
                    },
                    "anchor_text": {
                        "type": "string",
                        "description": "Unique text anchor to insert relative to"
                    },
                    "content": {
                        "type": "string",
                        "description": "Text content to insert"
                    },
                    "position": {
                        "type": "string",
                        "description": "Insert 'before' or 'after' the anchor_text",
                        "enum": ["before", "after"]
                    }
                },
                "required": ["path", "anchor_text", "content", "position"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let (path_str, anchor, content, position) = match parse_insert_args(&args) {
            Ok(values) => values,
            Err(result) => return result,
        };
        let canonical = match resolve_write_path(Path::new(path_str.as_str()), &self.allowed_dirs) {
            Ok(p) => p,
            Err(msg) => return result_msg(msg, true),
        };
        let existing = match tokio::fs::read_to_string(&canonical).await {
            Ok(c) => c,
            Err(_) => return result_msg("read error: permission denied", true),
        };

        // Check anchor existence
        let count = count_occurrences(&existing, &anchor);
        if count == 0 {
            return result_msg(format!("anchor_text '{}' not found in file", anchor), false);
        }
        if count > 1 {
            return result_msg(
                format!(
                    "anchor_text '{}' is not unique (found {} occurrences); please be more specific",
                    anchor, count
                ),
                false,
            );
        }

        let pos = existing.find(&anchor).unwrap();
        let new_content = match position.as_str() {
            "before" => {
                format!("{}{}{}", &existing[..pos], content, &existing[pos..])
            }
            "after" => {
                let after_pos = pos + anchor.len();
                format!(
                    "{}{}{}",
                    &existing[..after_pos],
                    content,
                    &existing[after_pos..]
                )
            }
            _ => unreachable!("validated position"),
        };

        match tokio::fs::write(&canonical, new_content.as_bytes()).await {
            Ok(()) => result_msg("inserted", false),
            Err(_) => result_msg("write error: permission denied", true),
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

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

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

fn parse_insert_args(
    args: &serde_json::Value,
) -> Result<(FilePath, String, String, String), ToolCallResult> {
    let path = match args["path"].as_str() {
        Some(p) if !p.is_empty() => FilePath::new(p),
        _ => {
            return Err(result_msg("missing or empty 'path' argument", true));
        }
    };
    let anchor = match args["anchor_text"].as_str() {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => {
            return Err(result_msg("missing or empty 'anchor_text' argument", true));
        }
    };
    let content = match args["content"].as_str() {
        Some(s) => s.to_owned(),
        None => {
            return Err(result_msg("missing 'content' argument", true));
        }
    };
    let position = match args["position"].as_str() {
        Some("before") | Some("after") => args["position"].as_str().unwrap().to_owned(),
        _ => {
            return Err(result_msg(
                "missing or invalid 'position' argument (must be 'before' or 'after')",
                true,
            ));
        }
    };
    Ok((path, anchor, content, position))
}
