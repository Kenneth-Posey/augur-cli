//! Built-in list_directory tool: lists files and subdirectories.
//!
//! Only paths within the configured allowed directories are accessible.
//! The requested directory is canonicalized and checked against the
//! `allowed_dirs` sandbox before any filesystem listing is performed.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::string_newtypes::{OutputText, StringNewtype, ToolName};
use augur_domain::tools::definition::ToolDefinition;
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "list_directory";
const INDENT_UNIT: &str = "  ";

/// Lists the contents of a directory, optionally walking subdirectories.
///
/// Only paths within the configured allowed directories are accessible.
/// Delegates path validation to the allowed-directory whitelist before listing.
pub struct ListDirectoryTool {
    allowed_dirs: Vec<PathBuf>,
    excluded_dirs: Vec<PathBuf>,
    excluded_dir_names: Vec<std::ffi::OsString>,
}

impl ListDirectoryTool {
    /// Create a new tool instance that restricts listings to `allowed_dirs` and excludes `excluded_dirs`.
    ///
    /// Each entry in `allowed_dirs` and `excluded_dirs` is canonicalized at construction time;
    /// entries that cannot be canonicalized are silently skipped.
    pub fn new(allowed_dirs: Vec<PathBuf>, excluded_dirs: Vec<PathBuf>) -> Self {
        let canonical_dirs = allowed_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        let excluded_dir_names = excluded_dirs
            .iter()
            .filter_map(|d| d.file_name().map(|name| name.to_os_string()))
            .collect();
        let canonical_excluded_dirs = excluded_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        ListDirectoryTool {
            allowed_dirs: canonical_dirs,
            excluded_dirs: canonical_excluded_dirs,
            excluded_dir_names,
        }
    }
}

#[derive(Clone, Copy)]
struct CollectConfig {
    depth: usize,
    recursive: bool,
}

#[derive(Clone, Copy)]
struct ListingExclusions<'a> {
    excluded_dirs: &'a [PathBuf],
    excluded_dir_names: &'a [std::ffi::OsString],
}

#[derive(Clone, Copy)]
struct CollectRequest<'a> {
    dir: &'a Path,
    config: CollectConfig,
}

#[derive(Clone, Copy)]
struct CollectEnvironment<'a> {
    request: CollectRequest<'a>,
    exclusions: ListingExclusions<'a>,
}

#[derive(Clone)]
struct ExecuteRequest {
    path: String,
    recursive: bool,
}

#[async_trait::async_trait]
impl ToolHandler for ListDirectoryTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "List the files and subdirectories in a directory. Set recursive=true to walk all subdirectories.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the directory to list"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "When true, recursively list all subdirectories. Default: false."
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(tool = "list_directory"))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        match execute_listing(self, args) {
            Ok(listing) => tool_result(OutputText::new(listing), false),
            Err(error) => tool_result(error, true),
        }
    }
}

fn execute_listing(
    tool: &ListDirectoryTool,
    args: serde_json::Value,
) -> Result<String, OutputText> {
    let request = parse_execute_request(args)?;
    let canonical = resolve_allowed_path(Path::new(&request.path), &tool.allowed_dirs)?;
    build_listing(
        &canonical,
        request.recursive,
        ListingExclusions {
            excluded_dirs: &tool.excluded_dirs,
            excluded_dir_names: &tool.excluded_dir_names,
        },
    )
    .map_err(|error| OutputText::new(error.to_string()))
}

fn parse_execute_request(args: serde_json::Value) -> Result<ExecuteRequest, OutputText> {
    let path = parse_path_argument(&args)?;
    let recursive = args["recursive"].as_bool().unwrap_or(false);
    Ok(ExecuteRequest { path, recursive })
}

fn parse_path_argument(args: &serde_json::Value) -> Result<String, OutputText> {
    match args["path"].as_str() {
        Some(path) if !path.is_empty() => Ok(path.to_owned()),
        _ => Err(OutputText::new("missing or empty 'path' argument")),
    }
}

fn tool_result(output: OutputText, is_error: bool) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(output)
        .is_error(augur_domain::domain::newtypes::IsPredicate::from(is_error))
        .build()
}

fn resolve_allowed_path(path: &Path, allowed_dirs: &[PathBuf]) -> Result<PathBuf, OutputText> {
    // Sandbox enforcement: canonicalize the requested path and verify it
    // falls within the configured allowed directories.
    let canonical =
        std::fs::canonicalize(path).map_err(|_| OutputText::new("list error: access denied"))?;
    if is_within_allowed_dirs(&canonical, allowed_dirs).is_none() {
        return Err(OutputText::new("list error: access denied"));
    }
    Ok(canonical)
}

/// Build a formatted directory listing for `path`.
///
/// Non-recursive: lists immediate entries. Recursive: walks the full subtree.
/// Each entry is indented 2 spaces per depth level. Directories are marked
/// with a trailing `/`. Entries are sorted: directories before files,
/// then alphabetically within each group.
fn build_listing(
    path: &Path,
    recursive: bool,
    exclusions: ListingExclusions<'_>,
) -> std::io::Result<String> {
    let mut lines: Vec<String> = Vec::new();
    let display_path = path.display().to_string();
    let root_label = if path.is_dir() {
        format!("{}/", display_path)
    } else {
        display_path.clone()
    };
    lines.push(root_label);
    collect_entries(
        CollectRequest {
            dir: path,
            config: CollectConfig {
                depth: 1,
                recursive,
            },
        },
        exclusions,
        &mut lines,
    )?;
    Ok(lines.join("\n"))
}

/// Recursively collect directory entries into `lines` with indentation.
///
/// Entries are sorted directories-first then alphabetically within each group.
/// Depth determines the leading spaces (2 spaces per depth level).
fn collect_entries(
    request: CollectRequest<'_>,
    exclusions: ListingExclusions<'_>,
    lines: &mut Vec<String>,
) -> std::io::Result<()> {
    let environment = CollectEnvironment {
        request,
        exclusions,
    };
    let entries = sorted_entries(environment.request.dir)?;
    for entry in entries.iter() {
        process_entry(entry, environment, lines)?;
    }
    Ok(())
}

fn sorted_entries(dir: &Path) -> std::io::Result<Vec<std::fs::DirEntry>> {
    let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .collect();
    entries.sort_by(compare_entries);
    Ok(entries)
}

fn compare_entries(a: &std::fs::DirEntry, b: &std::fs::DirEntry) -> std::cmp::Ordering {
    match (a.path().is_dir(), b.path().is_dir()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.file_name().cmp(&b.file_name()),
    }
}

fn process_entry(
    entry: &std::fs::DirEntry,
    environment: CollectEnvironment<'_>,
    lines: &mut Vec<String>,
) -> std::io::Result<()> {
    let entry_path = entry.path();
    if is_within_excluded_dirs(
        &entry_path,
        environment.exclusions.excluded_dirs,
        environment.exclusions.excluded_dir_names,
    ) {
        return Ok(());
    }
    let is_directory = entry_path.is_dir();
    lines.push(entry_label(
        entry,
        environment.request.config.depth,
        is_directory,
    ));
    if should_recurse(environment.request.config.recursive, is_directory) {
        recurse_into_directory(entry_path, environment, lines)?;
    }
    Ok(())
}

fn entry_label(entry: &std::fs::DirEntry, depth: usize, is_directory: bool) -> String {
    let indent = INDENT_UNIT.repeat(depth);
    let entry_name = entry.file_name().to_string_lossy().into_owned();
    if is_directory {
        return format!("{}{}/", indent, entry_name);
    }
    format!("{}{}", indent, entry_name)
}

fn should_recurse(recursive: bool, is_directory: bool) -> bool {
    recursive && is_directory
}

fn recurse_into_directory(
    entry_path: PathBuf,
    environment: CollectEnvironment<'_>,
    lines: &mut Vec<String>,
) -> std::io::Result<()> {
    collect_entries(
        CollectRequest {
            dir: &entry_path,
            config: CollectConfig {
                depth: environment.request.config.depth + 1,
                recursive: environment.request.config.recursive,
            },
        },
        environment.exclusions,
        lines,
    )
}

fn is_within_excluded_dirs(
    path: &Path,
    excluded_dirs: &[PathBuf],
    excluded_dir_names: &[std::ffi::OsString],
) -> bool {
    if let Some(name) = path.file_name()
        && excluded_dir_names.iter().any(|excluded| excluded == name)
    {
        return true;
    }
    if excluded_dirs
        .iter()
        .any(|excluded| path.starts_with(excluded))
    {
        return true;
    }
    if let Ok(canonical) = path.canonicalize() {
        return excluded_dirs
            .iter()
            .any(|excluded| canonical.starts_with(excluded));
    }
    false
}
