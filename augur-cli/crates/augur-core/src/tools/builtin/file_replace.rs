//! Built-in file_replace tool: replaces occurrences of old_text with new_text in a file.
//!
//! Supports optional start_text/end_text anchors to restrict replacements to a specific
//! text range. When anchors are provided, they must be unique in the file. Reports the
//! number of replacements made and notifies the LLM if old_text is not found.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "file_replace";

/// Replaces occurrences of old_text with new_text in a file.
///
/// When start_text and end_text are provided, restricts replacement to that
/// inclusive text range. Anchors must be unique when provided.
pub struct FileReplaceTool {
    allowed_dirs: Vec<PathBuf>,
}

impl FileReplaceTool {
    pub fn new(allowed_dirs: Vec<PathBuf>) -> Self {
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        FileReplaceTool {
            allowed_dirs: canonical_dirs,
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for FileReplaceTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Replace all occurrences of old_text with new_text in a file. \
             When start_text and end_text are provided, restricts replacement to \
             that inclusive text range. Anchors must be unique when provided. \
             Reports how many replacements were made.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file path"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "Text to replace"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "start_text": {
                        "type": "string",
                        "description": "Unique text marking the start of the range to replace (inclusive). Optional."
                    },
                    "end_text": {
                        "type": "string",
                        "description": "Unique text marking the end of the range to replace (inclusive). Optional."
                    }
                },
                "required": ["path", "old_text", "new_text"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let (path_str, old_text, new_text, range_text) = match parse_replace_args(&args) {
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

        // Check old_text existence
        let total_count = count_occurrences(&content, &old_text);
        if total_count == 0 {
            return result_msg("old_text not found in file", false);
        }

        let (new_content, replacements_made) = if let Some((start_text, end_text)) = &range_text {
            // Validate start_text
            let start_count = count_occurrences(&content, start_text);
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

            // Validate end_text
            let end_count = count_occurrences(&content, end_text);
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

            let start_pos = content.find(start_text.as_str()).unwrap();
            let end_pos = content.find(end_text.as_str()).unwrap();
            let end_exclusive = end_pos + end_text.len();

            if start_pos > end_pos {
                return result_msg(
                    "start_text appears after end_text in the file; cannot restrict range",
                    true,
                );
            }

            // Replace only within the range
            let before_range = &content[..start_pos];
            let range_content = &content[start_pos..end_exclusive];
            let after_range = &content[end_exclusive..];
            let range_count = count_occurrences(range_content, &old_text);
            let replaced = range_content.replace(&old_text, &new_text);
            (
                format!("{}{}{}", before_range, replaced, after_range),
                range_count,
            )
        } else {
            // Replace globally
            let new = content.replace(&old_text, &new_text);
            let count = if new != content { total_count } else { 0 };
            (new, count)
        };

        match tokio::fs::write(&canonical, new_content.as_bytes()).await {
            Ok(()) => result_msg(
                format!("replaced {} occurrence(s)", replacements_made),
                false,
            ),
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

#[allow(clippy::type_complexity)]
fn parse_replace_args(
    args: &serde_json::Value,
) -> Result<(FilePath, String, String, Option<(String, String)>), ToolCallResult> {
    let path = match args["path"].as_str() {
        Some(p) if !p.is_empty() => FilePath::new(p),
        _ => {
            return Err(result_msg("missing or empty 'path' argument", true));
        }
    };
    let old_text = match args["old_text"].as_str() {
        Some(s) if !s.is_empty() => s.to_owned(),
        _ => {
            return Err(result_msg("missing or empty 'old_text' argument", true));
        }
    };
    let new_text = match args["new_text"].as_str() {
        Some(s) => s.to_owned(),
        None => {
            return Err(result_msg("missing 'new_text' argument", true));
        }
    };
    // Parse optional range anchors
    let range = match (args["start_text"].as_str(), args["end_text"].as_str()) {
        (Some(s), Some(e)) if !s.is_empty() && !e.is_empty() => Some((s.to_owned(), e.to_owned())),
        (Some(s), None) if !s.is_empty() => {
            return Err(result_msg(
                "start_text provided but end_text is missing; provide both or neither",
                true,
            ));
        }
        (None, Some(e)) if !e.is_empty() => {
            return Err(result_msg(
                "end_text provided but start_text is missing; provide both or neither",
                true,
            ));
        }
        _ => None,
    };
    Ok((path, old_text, new_text, range))
}
