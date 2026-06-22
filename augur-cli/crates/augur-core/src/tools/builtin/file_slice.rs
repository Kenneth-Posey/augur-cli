//! Built-in file_slice tool: removes content between two unique text anchors (inclusive).
//!
//! Only paths within the configured allowed directories are accessible.
//! Finds the lines containing start_text and end_text, removes those lines and
//! everything between them (inclusive), and writes back.
//! Validates that both anchors are unique in the file before proceeding.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "file_slice";

/// Removes content between two unique text anchors (inclusive, line-based).
pub struct FileSliceTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileSliceTool {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        FileSliceTool {
            allowed_dirs: canonical_dirs,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileSliceTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Remove content between two unique text anchors (inclusive). \
             Both start_text and end_text must be unique in the file. \
             Removes entire lines containing the anchors and all lines between them.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file path"
                    },
                    "start_text": {
                        "type": "string",
                        "description": "Unique text marking the start of the range to remove (inclusive)"
                    },
                    "end_text": {
                        "type": "string",
                        "description": "Unique text marking the end of the range to remove (inclusive)"
                    }
                },
                "required": ["path", "start_text", "end_text"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let (path_str, start_text, end_text) = match parse_slice_args(&args) {
            Ok(values) => values,
            Err(result) => return result,
        };
        let canonical = match resolve_write_path(Path::new(path_str.as_str()), &self.allowed_dirs) {
            Ok(p) => p,
            Err(msg) => return result_msg(msg, true),
        };
        let content = match tokio::fs::read_to_string(&canonical).await {
            Ok(c) => c,
            Err(_) => return result_msg("read error: permission denied", true),
        };

        // Check start_text existence and uniqueness
        let start_count = count_occurrences(&content, &start_text);
        if start_count == 0 {
            return result_msg(
                format!("start_text '{}' not found in file", start_text),
                false,
            );
        }
        if start_count > 1 {
            return result_msg(
                format!(
                    "start_text '{}' is not unique (found {} occurrences); please be more specific",
                    start_text, start_count
                ),
                false,
            );
        }

        // Check end_text existence and uniqueness
        let end_count = count_occurrences(&content, &end_text);
        if end_count == 0 {
            return result_msg(format!("end_text '{}' not found in file", end_text), false);
        }
        if end_count > 1 {
            return result_msg(
                format!(
                    "end_text '{}' is not unique (found {} occurrences); please be more specific",
                    end_text, end_count
                ),
                false,
            );
        }

        // Find which lines contain start_text and end_text
        let lines: Vec<&str> = content.lines().collect();
        let start_line_idx = lines.iter().position(|l| l.contains(&start_text));
        let end_line_idx = lines.iter().position(|l| l.contains(&end_text));

        let (start_line, end_line) = match (start_line_idx, end_line_idx) {
            (Some(s), Some(e)) => (s, e),
            _ => {
                return result_msg(
                    "internal error: anchors found by count but not by line scan",
                    true,
                );
            }
        };

        if start_line > end_line {
            return result_msg(
                "start_text appears after end_text in the file; cannot slice",
                true,
            );
        }

        // Remove lines from start_line through end_line (inclusive)
        let mut result: Vec<&str> = Vec::with_capacity(lines.len());
        for (i, line) in lines.iter().enumerate() {
            if i < start_line || i > end_line {
                result.push(*line);
            }
        }

        let new_content = if result.is_empty() {
            String::new()
        } else if content.ends_with('\n') {
            format!("{}\n", result.join("\n"))
        } else {
            result.join("\n")
        };

        match tokio::fs::write(&canonical, new_content.as_bytes()).await {
            Ok(()) => result_msg("sliced", false),
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

fn parse_slice_args(
    args: &serde_json::Value,
) -> Result<(FilePath, String, String), ToolCallResult> {
    let path = match args["path"].as_str() {
        Some(p) if !p.is_empty() => FilePath::new(p),
        _ => {
            return Err(result_msg("missing or empty 'path' argument", true));
        }
    };
    let start_text = match args["start_text"].as_str() {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => {
            return Err(result_msg("missing or empty 'start_text' argument", true));
        }
    };
    let end_text = match args["end_text"].as_str() {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => {
            return Err(result_msg("missing or empty 'end_text' argument", true));
        }
    };
    Ok((path, start_text, end_text))
}
