//! Built-in `size_check` tool for safe file/directory sizing and scoped read-only probes.

use crate::tools::handler::{ToolCallResult, ToolHandler};
use crate::tools::ports::is_within_allowed_dirs;
use augur_domain::domain::newtypes::{IsPredicate, NumericNewtype};
use augur_domain::domain::string_newtypes::{FilePath, OutputText, StringNewtype, ToolName};
use augur_domain::domain::{ByteCount, TokenCount};
use augur_domain::tools::definition::ToolDefinition;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const TOOL_NAME: &str = "size_check";
const TOKEN_THRESHOLD_PROCEED: u64 = 10_000;
const TOKEN_THRESHOLD_FILTER: u64 = 50_000;
const TOKEN_THRESHOLD_PAGINATE: u64 = 100_000;
const MAX_COMMAND_OUTPUT_BYTES: u64 = 400_000;
const DEFAULT_MAX_DEPTH: u32 = 10;

/// Recommendation emitted from [`SizeCheckResponse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecommendationType {
    /// Safe to proceed with the original operation.
    Proceed,
    /// Narrow the query with a filter first.
    Filter,
    /// Read in pages/ranges/chunks.
    Paginate,
    /// Split into multiple smaller operations.
    Split,
}

/// Request payload for size checks.
#[derive(Clone, Debug, Deserialize, bon::Builder)]
pub struct SizeCheckRequest {
    /// Path to inspect (file or directory).
    pub path: FilePath,
    /// Optional command probe type (`ls`, `grep`, `find`, `du`, `wc`).
    #[serde(default)]
    pub command_type: Option<String>,
    /// Optional command-specific filter pattern.
    #[serde(default)]
    pub filter_pattern: Option<String>,
    /// Optional recursion depth for directory scans (`1..=100`).
    #[serde(default)]
    pub max_depth: Option<u32>,
}

/// Size-check result returned to the LLM.
#[derive(Clone, Debug, Serialize, Deserialize, bon::Builder)]
pub struct SizeCheckResponse {
    /// Canonical path that was inspected.
    pub path: FilePath,
    /// Total measured bytes.
    pub byte_count: ByteCount,
    /// Optional line/file counters from the size probe.
    #[serde(flatten)]
    pub counts: SizeCheckCounts,
    /// Estimated token count (heuristic).
    pub estimated_tokens: TokenCount,
    /// Guidance to keep future tool calls bounded.
    pub recommendation: RecommendationType,
}

/// Optional counters returned from a size probe.
#[derive(Clone, Debug, Default, Serialize, Deserialize, bon::Builder)]
pub struct SizeCheckCounts {
    /// Optional line count for text-like inputs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<u64>,
    /// Optional file count for directory scans.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<u64>,
}

/// Error type for `size_check`.
#[derive(Clone, Debug)]
pub enum SizeCheckError {
    /// Path invalid or outside allowed scope.
    InvalidPath(String),
    /// Unknown or blocked command.
    InvalidCommand(String),
    /// The target path does not exist.
    FileNotFound,
    /// Filesystem permission denied.
    PermissionDenied,
    /// Invalid command pattern/filter.
    InvalidPattern,
    /// Command output exceeded safety limit.
    OutputTooLarge,
    /// Command execution failed.
    ExecutionFailed(String),
    /// Generic IO failure.
    IoError(String),
}

impl std::fmt::Display for SizeCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SizeCheckError::InvalidPath(msg) => write!(f, "Invalid path: {msg}"),
            SizeCheckError::InvalidCommand(cmd) => write!(f, "Invalid command: {cmd}"),
            SizeCheckError::FileNotFound => write!(f, "File or directory not found"),
            SizeCheckError::PermissionDenied => write!(f, "Permission denied"),
            SizeCheckError::InvalidPattern => write!(f, "Invalid filter pattern"),
            SizeCheckError::OutputTooLarge => write!(f, "Command output exceeded size limit"),
            SizeCheckError::ExecutionFailed(msg) => write!(f, "Execution failed: {msg}"),
            SizeCheckError::IoError(msg) => write!(f, "IO error: {msg}"),
        }
    }
}

impl std::error::Error for SizeCheckError {}

/// Bundles excluded directory paths and names to reduce parameter counts
/// in functions that need both exclusion inputs.
///
/// This is the same data that `SizeCheckTool` stores as two separate vecs.
/// Passing an `ExclusionConfig` avoids repeating the pair in function signatures.
#[derive(Clone, Copy, Debug)]
pub struct ExclusionConfig<'a> {
    /// Canonical paths to exclude from scans.
    pub excluded_dirs: &'a [PathBuf],
    /// Directory base names to exclude (matched by `file_name`).
    pub excluded_dir_names: &'a [OsString],
}

impl<'a> ExclusionConfig<'a> {
    /// Create a new exclusion configuration from the two exclusion collections.
    pub const fn new(excluded_dirs: &'a [PathBuf], excluded_dir_names: &'a [OsString]) -> Self {
        Self {
            excluded_dirs,
            excluded_dir_names,
        }
    }
}

/// Tool handler that exposes `size_check` to the LLM runtime.
///
/// Enforces both allowed-directory sandboxing and excluded-directory filtering.
/// Excluded directories are skipped during recursive directory walking so the
/// model does not receive size estimates for content inside `.git`, `target`,
/// `changelogs`, `logs/`, or any user-configured excluded paths.
pub struct SizeCheckTool {
    allowed_dirs: Vec<PathBuf>,
    excluded_dirs: Vec<PathBuf>,
    excluded_dir_names: Vec<OsString>,
}

impl SizeCheckTool {
    /// Create a size-check tool sandboxed to `allowed_dirs` and excluding `excluded_dirs`.
    ///
    /// Each entry in `allowed_dirs` and `excluded_dirs` is canonicalized at construction time;
    /// entries that cannot be canonicalized are silently skipped.
    pub fn new(allowed_dirs: Vec<PathBuf>, excluded_dirs: Vec<PathBuf>) -> Self {
        let allowed_dirs = allowed_dirs
            .into_iter()
            .filter_map(|dir| dir.canonicalize().ok())
            .collect();
        let excluded_dir_names = excluded_dirs
            .iter()
            .filter_map(|d| d.file_name().map(|name| name.to_os_string()))
            .collect();
        let canonical_excluded_dirs = excluded_dirs
            .into_iter()
            .filter_map(|d| d.canonicalize().ok())
            .collect();
        Self {
            allowed_dirs,
            excluded_dirs: canonical_excluded_dirs,
            excluded_dir_names,
        }
    }

    /// Build the [`ExclusionConfig`] from this tool's stored exclusion data.
    fn exclusion_config(&self) -> ExclusionConfig<'_> {
        ExclusionConfig::new(&self.excluded_dirs, &self.excluded_dir_names)
    }
}

#[async_trait::async_trait]
impl ToolHandler for SizeCheckTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            TOOL_NAME,
            "Check file/directory size or safe read-only command output to decide whether to proceed, filter, paginate, or split before large operations.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative file/directory path"
                    },
                    "command_type": {
                        "type": "string",
                        "enum": ["ls", "grep", "find", "du", "wc"],
                        "description": "Optional read-only command probe"
                    },
                    "filter_pattern": {
                        "type": "string",
                        "description": "Optional command filter pattern"
                    },
                    "max_depth": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "description": "Optional recursion depth for directory scan"
                    }
                },
                "required": ["path"]
            }),
        )
    }

    #[tracing::instrument(skip(self, args), fields(tool = "size_check"))]
    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        let request = match serde_json::from_value::<SizeCheckRequest>(args) {
            Ok(request) => request,
            Err(error) => {
                return ToolCallResult::builder()
                    .name(ToolName::new(TOOL_NAME))
                    .output(OutputText::new(format!("invalid size_check args: {error}")))
                    .is_error(IsPredicate::from(true))
                    .build();
            }
        };
        match check_size_with_scope(request, &self.allowed_dirs, self.exclusion_config()) {
            Ok(response) => {
                let output = serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|error| format!("size_check serialization failure: {error}"));
                ToolCallResult::builder()
                    .name(ToolName::new(TOOL_NAME))
                    .output(OutputText::new(output))
                    .is_error(IsPredicate::from(false))
                    .build()
            }
            Err(error) => ToolCallResult::builder()
                .name(ToolName::new(TOOL_NAME))
                .output(OutputText::new(error.to_string()))
                .is_error(IsPredicate::from(true))
                .build(),
        }
    }
}

/// Run `size_check` constrained to canonical `allowed_dirs` and excluding
/// directories identified by `exclusions`.
pub fn check_size_with_scope(
    request: SizeCheckRequest,
    allowed_dirs: &[PathBuf],
    exclusions: ExclusionConfig<'_>,
) -> Result<SizeCheckResponse, SizeCheckError> {
    validate_max_depth(request.max_depth)?;
    let canonical_path = canonicalize_path(Path::new(request.path.as_str()), allowed_dirs)?;
    let probe = size_probe(
        &canonical_path,
        SizeProbeOptions::builder()
            .maybe_command_type(request.command_type.as_deref())
            .maybe_filter_pattern(request.filter_pattern.as_deref())
            .maybe_max_depth(request.max_depth)
            .build(),
        exclusions,
    )?;
    let estimated_tokens = estimate_tokens(probe.byte_count);
    Ok(SizeCheckResponse::builder()
        .path(FilePath::new(canonical_path.to_string_lossy().to_string()))
        .byte_count(ByteCount::from(probe.byte_count))
        .counts(
            SizeCheckCounts::builder()
                .maybe_line_count(probe.line_count)
                .maybe_file_count(probe.file_count)
                .build(),
        )
        .estimated_tokens(estimated_tokens)
        .recommendation(recommendation_for_tokens(estimated_tokens.inner()))
        .build())
}

#[derive(Clone, Copy, Debug, bon::Builder)]
struct ProbeResult {
    byte_count: u64,
    line_count: Option<u64>,
    file_count: Option<u64>,
}

#[derive(Clone, Copy, Debug, bon::Builder)]
struct SizeProbeOptions<'a> {
    command_type: Option<&'a str>,
    filter_pattern: Option<&'a str>,
    max_depth: Option<u32>,
}

fn size_probe(
    canonical_path: &Path,
    options: SizeProbeOptions<'_>,
    exclusions: ExclusionConfig<'_>,
) -> Result<ProbeResult, SizeCheckError> {
    match options.command_type {
        Some(command) => {
            validate_command_is_whitelisted(command)?;
            let (byte_count, line_count) =
                execute_read_only_command(command, canonical_path, options.filter_pattern)?;
            Ok(ProbeResult::builder()
                .byte_count(byte_count)
                .maybe_line_count(Some(line_count))
                .build())
        }
        None => {
            if canonical_path.is_file() {
                let (byte_count, line_count) = check_file_size(canonical_path)?;
                return Ok(ProbeResult::builder()
                    .byte_count(byte_count)
                    .maybe_line_count(line_count)
                    .build());
            }
            if canonical_path.is_dir() {
                let (byte_count, file_count) =
                    check_dir_size(canonical_path, options.max_depth, exclusions)?;
                return Ok(ProbeResult::builder()
                    .byte_count(byte_count)
                    .maybe_file_count(Some(file_count))
                    .build());
            }
            Err(SizeCheckError::FileNotFound)
        }
    }
}

fn validate_max_depth(max_depth: Option<u32>) -> Result<(), SizeCheckError> {
    if let Some(depth) = max_depth
        && !(1..=100).contains(&depth)
    {
        return Err(SizeCheckError::InvalidPath(
            "max_depth must be in range 1..=100".to_owned(),
        ));
    }
    Ok(())
}

fn check_file_size(path: &Path) -> Result<(u64, Option<u64>), SizeCheckError> {
    let metadata = std::fs::metadata(path).map_err(SizeCheckError::from)?;
    let byte_count = metadata.len();
    if !is_text_file(path)? {
        return Ok((byte_count, None));
    }
    Ok((byte_count, Some(count_lines(path)?)))
}

fn is_text_file(path: &Path) -> Result<bool, SizeCheckError> {
    let bytes = std::fs::read(path).map_err(SizeCheckError::from)?;
    let sample = bytes.get(..512).unwrap_or(&bytes);
    Ok(!sample.contains(&0))
}

fn count_lines(path: &Path) -> Result<u64, SizeCheckError> {
    let file = std::fs::File::open(path).map_err(SizeCheckError::from)?;
    let mut reader = BufReader::new(file);
    let mut line_count = 0u64;
    let mut buf = String::new();
    loop {
        buf.clear();
        if reader.read_line(&mut buf).map_err(SizeCheckError::from)? == 0 {
            break;
        }
        line_count += 1;
    }
    Ok(line_count)
}

fn check_dir_size(
    path: &Path,
    max_depth: Option<u32>,
    exclusions: ExclusionConfig<'_>,
) -> Result<(u64, u64), SizeCheckError> {
    let mut totals = DirectoryTotals::default();
    let mut traversal = DirectoryTraversal::builder()
        .max_depth(max_depth.unwrap_or(DEFAULT_MAX_DEPTH))
        .totals(&mut totals)
        .excluded_dirs(exclusions.excluded_dirs)
        .excluded_dir_names(exclusions.excluded_dir_names)
        .build();
    walk_dir_recursive(path, 0, &mut traversal)?;
    Ok((totals.total_bytes, totals.file_count))
}

#[derive(Default)]
struct DirectoryTotals {
    total_bytes: u64,
    file_count: u64,
}

#[derive(bon::Builder)]
struct DirectoryTraversal<'a> {
    max_depth: u32,
    totals: &'a mut DirectoryTotals,
    excluded_dirs: &'a [PathBuf],
    excluded_dir_names: &'a [OsString],
}

fn walk_dir_recursive(
    dir: &Path,
    current_depth: u32,
    traversal: &mut DirectoryTraversal<'_>,
) -> Result<(), SizeCheckError> {
    if current_depth >= traversal.max_depth {
        return Ok(());
    }
    for entry_result in std::fs::read_dir(dir).map_err(SizeCheckError::from)? {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        // Skip entries that match an excluded directory name or path.
        if is_excluded(&path, traversal.excluded_dirs, traversal.excluded_dir_names) {
            continue;
        }
        if path.is_file() {
            if let Ok(metadata) = std::fs::metadata(&path) {
                traversal.totals.total_bytes += metadata.len();
                traversal.totals.file_count += 1;
            }
            continue;
        }
        if path.is_dir() {
            let _ = walk_dir_recursive(&path, current_depth + 1, traversal);
        }
    }
    Ok(())
}

/// Returns `true` when `path` matches an excluded directory name or is
/// beneath an excluded canonical path.
fn is_excluded(path: &Path, excluded_dirs: &[PathBuf], excluded_dir_names: &[OsString]) -> bool {
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

fn execute_read_only_command(
    command: &str,
    path: &Path,
    filter_pattern: Option<&str>,
) -> Result<(u64, u64), SizeCheckError> {
    let args = build_command_args(command, path, filter_pattern)?;
    for arg in &args {
        sanitize_command_arg(arg)?;
    }
    let output = crate::tools::builtin::child_process::piped_command_sync(command)
        .args(&args)
        .output()
        .map_err(|error| SizeCheckError::ExecutionFailed(error.to_string()))?;
    if !output.status.success() {
        return Err(SizeCheckError::ExecutionFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    let output_bytes = output.stdout.len() as u64;
    if output_bytes > MAX_COMMAND_OUTPUT_BYTES {
        return Err(SizeCheckError::OutputTooLarge);
    }
    Ok((output_bytes, count_lines_in_bytes(&output.stdout)))
}

fn build_command_args(
    command: &str,
    path: &Path,
    filter_pattern: Option<&str>,
) -> Result<Vec<String>, SizeCheckError> {
    let canonical = path.to_string_lossy().to_string();
    match command {
        "ls" => Ok(vec!["-la".to_owned(), canonical]),
        "grep" => {
            let pattern = filter_pattern.ok_or(SizeCheckError::InvalidPattern)?;
            if path.is_dir() {
                return Ok(vec!["-R".to_owned(), pattern.to_owned(), canonical]);
            }
            Ok(vec![pattern.to_owned(), canonical])
        }
        "find" => {
            let mut args = vec![canonical];
            if let Some(pattern) = filter_pattern {
                args.push("-name".to_owned());
                args.push(pattern.to_owned());
            }
            Ok(args)
        }
        "du" => Ok(vec!["-sh".to_owned(), canonical]),
        "wc" => Ok(vec!["-l".to_owned(), canonical]),
        _ => Err(SizeCheckError::InvalidCommand(command.to_owned())),
    }
}

fn count_lines_in_bytes(output: &[u8]) -> u64 {
    output.iter().filter(|&&byte| byte == b'\n').count() as u64
}

fn canonicalize_path(path: &Path, allowed_dirs: &[PathBuf]) -> Result<PathBuf, SizeCheckError> {
    let canonical = std::fs::canonicalize(path).map_err(SizeCheckError::from)?;
    if !allowed_dirs.is_empty() && is_within_allowed_dirs(&canonical, allowed_dirs).is_none() {
        return Err(SizeCheckError::InvalidPath(
            "path escapes allowed scope".to_owned(),
        ));
    }
    Ok(canonical)
}

fn validate_command_is_whitelisted(command: &str) -> Result<(), SizeCheckError> {
    match command {
        "ls" | "grep" | "find" | "du" | "wc" => Ok(()),
        _ => Err(SizeCheckError::InvalidCommand(command.to_owned())),
    }
}

fn sanitize_command_arg(arg: &str) -> Result<(), SizeCheckError> {
    let dangerous = ['$', '`', '|', '&', ';', '>', '<', '*', '?', '\'', '"'];
    if let Some(ch) = dangerous.iter().copied().find(|ch| arg.contains(*ch)) {
        return Err(SizeCheckError::InvalidCommand(format!(
            "dangerous character '{ch}' in argument"
        )));
    }
    Ok(())
}

fn estimate_tokens(byte_count: u64) -> TokenCount {
    TokenCount::from(byte_count / 4)
}

fn recommendation_for_tokens(estimated_tokens: u64) -> RecommendationType {
    if estimated_tokens < TOKEN_THRESHOLD_PROCEED {
        return RecommendationType::Proceed;
    }
    if estimated_tokens <= TOKEN_THRESHOLD_FILTER {
        return RecommendationType::Filter;
    }
    if estimated_tokens <= TOKEN_THRESHOLD_PAGINATE {
        return RecommendationType::Paginate;
    }
    RecommendationType::Split
}

impl From<std::io::Error> for SizeCheckError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => SizeCheckError::FileNotFound,
            std::io::ErrorKind::PermissionDenied => SizeCheckError::PermissionDenied,
            _ => SizeCheckError::IoError(error.to_string()),
        }
    }
}
