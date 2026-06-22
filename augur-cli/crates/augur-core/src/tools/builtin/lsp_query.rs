//! `lsp_query` built-in tool - routes LSP queries via the `LspClient` port.
//!
//! Validates input, maps each operation to a JSON-RPC request, awaits the
//! reply from the actor, and formats results as human-readable text.

use crate::tools::handler::ToolHandler;
use augur_domain::domain::lsp::{LspError, LspLocation, LspOperation, LspQueryInput, LspSymbol};
use augur_domain::domain::newtypes::{CharacterOffset, Count, IsPredicate, LineNumber};
use augur_domain::domain::string_newtypes::{
    OutputText, RootUri, StringNewtype, ToolDescription, ToolName,
};
use augur_domain::domain::tool_types::{ToolCallResult, ToolDefinition};
use augur_domain::domain::traits::LspClient;
use std::time::Duration;

// ── Constants ─────────────────────────────────────────────────────────────────

const TOOL_NAME: &str = "lsp_query";

/// Timeout in seconds for LSP requests; exported for test inspection.
pub(super) const LSP_REQUEST_TIMEOUT_SECS: u64 = 30;

/// Maximum Unicode scalar values per code snippet; longer lines are truncated.
const SNIPPET_MAX_CHARS: usize = 120;

/// LSP `SymbolKind` index → name table (0-based, index 0 is the "unknown" sentinel).
const SYMBOL_KIND_NAMES: &[&str] = &[
    "Unknown",       // 0 - not in LSP spec
    "File",          // 1
    "Module",        // 2
    "Namespace",     // 3
    "Package",       // 4
    "Class",         // 5
    "Method",        // 6
    "Property",      // 7
    "Field",         // 8
    "Constructor",   // 9
    "Enum",          // 10
    "Interface",     // 11
    "Function",      // 12
    "Variable",      // 13
    "Constant",      // 14
    "String",        // 15
    "Number",        // 16
    "Boolean",       // 17
    "Array",         // 18
    "Object",        // 19
    "Key",           // 20
    "Null",          // 21
    "EnumMember",    // 22
    "Struct",        // 23
    "Event",         // 24
    "Operator",      // 25
    "TypeParameter", // 26
];

// ── Public struct and constructor ─────────────────────────────────────────────

/// `lsp_query` tool implementation backed by an `LspClient`.
///
/// Wraps the handle so it can be stored in an `Arc<dyn ToolHandler>` registry.
///
/// # Invariants
///
pub struct LspQueryTool {
    handle: std::sync::Arc<dyn LspClient>,
}

impl LspQueryTool {
    /// Construct a new `LspQueryTool` backed by the given LSP client.
    pub fn new(handle: impl LspClient) -> Self {
        LspQueryTool {
            handle: std::sync::Arc::new(handle),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for LspQueryTool {
    fn definition(&self) -> ToolDefinition {
        definition()
    }

    async fn execute(&self, args: serde_json::Value) -> ToolCallResult {
        execute(self.handle.as_ref(), args).await
    }
}

// ── Public free functions (also used by tests via `use super::*`) ─────────────

/// Return the tool schema definition used for LLM tool registration.
///
/// # Returns
///
/// A [`ToolDefinition`] with name `"lsp_query"`, a short description, and a
/// JSON Schema object with five properties: `operation`, `file_path`, `line`,
/// `character`, `query` (only `operation` required).
///
/// # Invariants
///
/// - `ToolDefinition::name()` returns `"lsp_query"`.
/// - The JSON Schema is valid (object type, properties map, required list).
pub fn definition() -> ToolDefinition {
    ToolDefinition::new(
        ToolName::new(TOOL_NAME),
        ToolDescription::new(
            "Query rust-analyzer for code intelligence: go-to-definition, \
             find-references, hover, document symbols, workspace symbols, \
             go-to-implementation, find-callers, or rename.",
        ),
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "description": "LSP operation: goToDefinition, findReferences, hover, documentSymbol, workspaceSymbol, goToImplementation, findCallers, rename"
                },
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the source file (required for all position operations and documentSymbol)"
                },
                "line": {
                    "type": "integer",
                    "description": "Zero-based line number within the file (optional for position operations when symbol_name is provided)"
                },
                "character": {
                    "type": "integer",
                    "description": "Zero-based character offset within the line (optional for position operations when symbol_name is provided)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query string (required for workspaceSymbol)"
                },
                "symbol_name": {
                    "type": "string",
                    "description": "Symbol name to resolve internally, alternative to providing exact line/character coordinates (optional for position operations)"
                },
                "new_name": {
                    "type": "string",
                    "description": "New name for the symbol being renamed (required for rename operation)"
                }
            },
            "required": ["operation"]
        }),
    )
}

/// Execute an LSP query described by `input_value`.
///
/// Validates the raw `serde_json::Value` arguments, dispatches the matching
/// LSP operation to the actor via `handle`, waits up to 10 s for a reply, and
/// returns a [`ToolCallResult`].
///
/// Always returns `Ok(...)` - all error conditions are encoded as an
/// `is_error: true` result with the error description as output text.
///
/// # Preconditions
///
/// - `handle` is a live `LspClient`.
/// - `input_value` is the raw arguments from the LLM tool call.
///
/// # Postconditions
///
/// - Returned `ToolCallResult.session_log` is always `Some(...)`.
/// - `is_error` is `true` iff an error occurred.
pub async fn execute(handle: &dyn LspClient, input_value: serde_json::Value) -> ToolCallResult {
    match validate_input(&input_value).await {
        Err(err_result) => err_result,
        Ok(query_input) => dispatch_operation(handle, &query_input).await,
    }
}

/// Validate `workspaceSymbol` args: requires `query` string.
fn validate_symbol_args(
    op: &str,
    args: &serde_json::Value,
) -> Result<LspQueryInput, ToolCallResult> {
    let query = match args["query"].as_str() {
        Some(q) => q.to_owned(),
        None => return Err(make_error_result(op, "missing 'query'")),
    };
    Ok(LspQueryInput::SymbolQuery { query })
}

/// Validate `documentSymbol` args: requires non-empty `file_path` that exists on disk.
async fn validate_file_arg(
    op: &str,
    args: &serde_json::Value,
) -> Result<LspQueryInput, ToolCallResult> {
    let file_path = match args["file_path"].as_str().filter(|s| !s.is_empty()) {
        Some(p) => p.to_owned(),
        None => {
            return Err(make_error_result(
                op,
                "missing or invalid 'file_path' argument",
            ))
        }
    };
    if let Err(msg) = check_file_exists(&file_path).await {
        return Err(make_error_result(op, &msg));
    }
    Ok(LspQueryInput::FileQuery { file_path })
}

/// Validate `rename` args: requires `file_path`, `new_name`, and either `line`+`character`
/// or `symbol_name`.
async fn validate_rename_args(
    op: &str,
    args: &serde_json::Value,
) -> Result<LspQueryInput, ToolCallResult> {
    let file_path = parse_required_string_arg(op, args, "file_path")?;
    ensure_position_file_exists(op, &file_path).await?;
    let new_name = parse_required_string_arg(op, args, "new_name")?;

    let symbol_name = args["symbol_name"].as_str().map(|s| s.to_owned());

    if symbol_name.is_some() {
        let line = args["line"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        let character = args["character"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        return Ok(LspQueryInput::RenameQuery {
            file_path,
            line,
            character,
            new_name,
        });
    }

    let line = parse_u32_arg(op, args, "line")?;
    let character = parse_u32_arg(op, args, "character")?;
    Ok(LspQueryInput::RenameQuery {
        file_path,
        line,
        character,
        new_name,
    })
}

/// Validate position-operation args: requires `file_path`, and either `line`+`character`
/// or `symbol_name`.
async fn validate_position_args(
    op: &str,
    args: &serde_json::Value,
) -> Result<LspQueryInput, ToolCallResult> {
    let file_path = parse_required_string_arg(op, args, "file_path")?;
    ensure_position_file_exists(op, &file_path).await?;

    let symbol_name = args["symbol_name"].as_str().map(|s| s.to_owned());

    // When symbol_name is provided, line/character are optional and will be
    // resolved internally. When not provided, both are required.
    if symbol_name.is_some() {
        let line = args["line"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        let character = args["character"]
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        return Ok(LspQueryInput::PositionQuery {
            operation: position_operation(op),
            file_path,
            line,
            character,
            symbol_name,
        });
    }

    let line = parse_u32_arg(op, args, "line")?;
    let character = parse_u32_arg(op, args, "character")?;
    Ok(LspQueryInput::PositionQuery {
        operation: position_operation(op),
        file_path,
        line,
        character,
        symbol_name,
    })
}

struct PositionQueryArgs {
    file_path: String,
    line: u32,
    character: u32,
}

fn parse_position_query_args(
    op: &str,
    args: &serde_json::Value,
) -> Result<PositionQueryArgs, ToolCallResult> {
    Ok(PositionQueryArgs {
        file_path: parse_required_string_arg(op, args, "file_path")?,
        line: parse_u32_arg(op, args, "line")?,
        character: parse_u32_arg(op, args, "character")?,
    })
}

async fn ensure_position_file_exists(op: &str, file_path: &str) -> Result<(), ToolCallResult> {
    check_file_exists(file_path)
        .await
        .map_err(|msg| make_error_result(op, &msg))
}

fn parse_required_string_arg(
    op: &str,
    args: &serde_json::Value,
    field: &str,
) -> Result<String, ToolCallResult> {
    args[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| make_error_result(op, &format!("missing or invalid '{field}' argument")))
}

fn parse_u32_arg(op: &str, args: &serde_json::Value, field: &str) -> Result<u32, ToolCallResult> {
    args[field]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| make_error_result(op, &format!("missing or invalid '{field}' argument")))
}

fn position_operation(op: &str) -> LspOperation {
    match op {
        "goToDefinition" => LspOperation::GoToDefinition,
        "findReferences" => LspOperation::FindReferences,
        "goToImplementation" => LspOperation::GoToImplementation,
        "findCallers" => LspOperation::FindCallers,
        _ => LspOperation::Hover,
    }
}

/// Validate and parse the raw arguments from the LLM into a typed [`LspQueryInput`].
///
/// Performs file-existence checks for operations that require a `file_path`.
///
/// # Errors
///
/// Returns `Err(ToolCallResult)` with `is_error=true` for any validation failure.
pub(super) async fn validate_input(
    args: &serde_json::Value,
) -> Result<LspQueryInput, ToolCallResult> {
    let op = args["operation"].as_str().unwrap_or("").to_owned();
    match op.as_str() {
        "workspaceSymbol" => validate_symbol_args(&op, args),
        "documentSymbol" => validate_file_arg(&op, args).await,
        "rename" => validate_rename_args(&op, args).await,
        "goToDefinition" | "findReferences" | "hover" | "goToImplementation" | "findCallers" => {
            validate_position_args(&op, args).await
        }
        other => Err(make_error_result(
            other,
            &format!(
                "unknown operation '{other}'; valid values: goToDefinition, findReferences, \
                 hover, documentSymbol, workspaceSymbol, goToImplementation, findCallers, rename",
            ),
        )),
    }
}

/// Return the `make_session_log` result (exported for tests via `use super::*`).
pub(super) fn make_session_log(op: &OutputText, count: Option<Count>) -> OutputText {
    let s = match count {
        Some(n) => format!("lsp_query {}: {} result(s)", op, n),
        None => format!("lsp_query {}: error", op),
    };
    OutputText::new(s)
}

/// Convert the raw actor reply (or `LspError`) into `Ok(Value)` or `Err(String)`.
///
/// - `Err(e)` → `Err(e.to_string())`
/// - `Ok(value)` where `value["error"].is_object()` → `Err("lsp error {code}: {message}")`
/// - `Ok(value)` otherwise → `Ok(value)`
pub(super) fn handle_lsp_response(
    result: Result<serde_json::Value, LspError>,
) -> Result<serde_json::Value, OutputText> {
    match result {
        Err(e) => Err(OutputText::new(e.to_string())),
        Ok(v) if v["error"].is_object() => {
            let code = v["error"]["code"].as_i64().unwrap_or(0);
            let msg = v["error"]["message"].as_str().unwrap_or("").to_owned();
            Err(OutputText::new(format!("lsp error {}: {}", code, msg)))
        }
        Ok(v) => Ok(v),
    }
}

/// Format a list of [`LspLocation`]s as human-readable text with code snippets.
///
/// Each location is formatted as `"{uri}:{line+1}:{char+1}"`, optionally
/// followed by two spaces and the trimmed source snippet at that line (up to
/// 120 code points, truncated with `U+2026` if longer). Entries are joined
/// with `"\n"` (no trailing newline). Returns `""` for an empty slice.
///
/// # Postconditions
///
/// - Empty slice → `""`.
/// - Each entry is `"coord"` or `"coord  snippet"` (two spaces before snippet).
/// - Lines ≤ 120 chars used verbatim; lines > 120 chars get appended `\u{2026}`.
pub(super) async fn format_locations(locations: &[LspLocation]) -> OutputText {
    let mut lines: Vec<String> = Vec::with_capacity(locations.len());
    for loc in locations {
        let coord = format!(
            "{}:{}:{}",
            loc.uri,
            loc.start_line + LineNumber::of(1),
            loc.start_character + CharacterOffset::of(1)
        );
        let snippet = read_snippet(&loc.uri, (*loc.start_line) as usize).await;
        let line = match snippet {
            Some(s) => format!("{}  {}", coord, s),
            None => coord,
        };
        lines.push(line);
    }
    OutputText::new(lines.join("\n"))
}

/// Format a list of [`LspSymbol`]s as `"{kind} {name}  {uri}:{start_line+1}"` per entry.
///
/// Entries joined with `"\n"` (no trailing newline). Returns `""` for empty slice.
pub(super) fn format_symbols(symbols: &[LspSymbol]) -> OutputText {
    OutputText::new(
        symbols
            .iter()
            .map(|s| {
                format!(
                    "{} {}  {}:{}",
                    s.kind,
                    s.name,
                    s.uri,
                    s.start_line + LineNumber::of(1)
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Flatten a JSON LSP document-symbol response into a `Vec<LspSymbol>`.
///
/// Handles both the `DocumentSymbol` format (has `"selectionRange"` field)
/// and the `SymbolInformation` format (has `"location"` field). Processes
/// items in depth-first pre-order (parent before children). Returns `vec![]`
/// for `null` or empty input.
pub(super) fn flatten_document_symbols(value: &serde_json::Value) -> Vec<LspSymbol> {
    let arr = match value.as_array() {
        None => return Vec::new(),
        Some(a) => a,
    };

    let mut result = Vec::new();
    for item in arr {
        flatten_symbol(item, "", &mut result);
    }
    result
}

// ── Symbol-name resolution ─────────────────────────────────────────────────────

/// If the `PositionQuery` has a `symbol_name` set, resolve it via
/// `workspace/symbol` to determine the correct `file_path`, `line`, and
/// `character`. Otherwise returns the input unchanged.
///
/// If multiple symbols match, picks the first result with the matching
/// `file_path` (when provided in the original input), or the first result
/// otherwise.
async fn resolve_symbol_name_if_needed(
    handle: &dyn LspClient,
    input: &LspQueryInput,
) -> LspQueryInput {
    let (file_path, line, character, operation, symbol_name) = match input {
        LspQueryInput::PositionQuery {
            file_path,
            line,
            character,
            operation,
            symbol_name: Some(name),
        } => (file_path, *line, *character, operation, name.clone()),
        _ => return input.clone(),
    };

    // Query workspace/symbol with the name
    let params = serde_json::json!({ "query": symbol_name });
    let raw = await_lsp_reply(handle, "workspace/symbol", params).await;

    let coord = match raw {
        Ok(v) => resolve_best_coordinate(&v, file_path),
        Err(_) => None,
    };

    match coord {
        Some((resolved_path, resolved_line, resolved_char)) => {
            LspQueryInput::PositionQuery {
                operation: operation.clone(),
                file_path: resolved_path,
                line: resolved_line,
                character: resolved_char,
                symbol_name: None, // resolved; clear the field
            }
        }
        None => {
            // Fall back to original input if resolution fails
            LspQueryInput::PositionQuery {
                operation: operation.clone(),
                file_path: file_path.clone(),
                line,
                character,
                symbol_name: Some(symbol_name),
            }
        }
    }
}

/// Find the best match in a workspace/symbol result set.
///
/// Prefers entries whose `uri` matches `file_path` (when provided). Returns
/// the first match's location, or `None` if the result set is empty.
fn resolve_best_coordinate(
    value: &serde_json::Value,
    file_path: &str,
) -> Option<(String, u32, u32)> {
    let items = value.as_array()?;
    // First pass: prefer files matching the requested file_path
    for item in items {
        let uri = item["location"]["uri"].as_str().unwrap_or("").to_owned();
        let path = uri.strip_prefix("file://").unwrap_or(&uri).to_owned();
        if (path == *file_path || uri == *file_path)
            && let Some(coord) = extract_location_coord(item)
        {
            return Some((path, coord.0, coord.1));
        }
    }
    // Second pass: take the first result with valid coordinates
    for item in items {
        if let Some(coord) = extract_location_coord(item) {
            let uri = item["location"]["uri"].as_str().unwrap_or("").to_owned();
            let path = uri.strip_prefix("file://").unwrap_or(&uri).to_owned();
            return Some((path, coord.0, coord.1));
        }
    }
    None
}

fn extract_location_coord(item: &serde_json::Value) -> Option<(u32, u32)> {
    let line = item["location"]["range"]["start"]["line"].as_u64()? as u32;
    let character = item["location"]["range"]["start"]["character"].as_u64()? as u32;
    Some((line, character))
}

// ── New operation handlers ────────────────────────────────────────────────────

/// Handle `goToImplementation` LSP query - find trait implementations.
async fn go_to_implementation(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::PositionQuery {
        file_path,
        line,
        character,
        ..
    } = input
    else {
        return make_error_result("goToImplementation", "internal: wrong input variant");
    };
    let op = "goToImplementation";
    let uri = format!("file://{}", file_path);
    let params = serde_json::json!({
        "textDocument": {"uri": uri},
        "position": {"line": line, "character": character}
    });

    let raw = await_lsp_reply(handle, "textDocument/implementation", params).await;

    match handle_lsp_response(raw) {
        Err(e) => make_error_result(op, e.as_ref()),
        Ok(v) => {
            let locations = parse_locations(&v);
            if locations.is_empty() {
                make_success_result(op, 0, "No implementations found".to_string())
            } else {
                let count = locations.len();
                let text = format_locations(&locations).await;
                make_success_result(op, count, text.as_str().to_owned())
            }
        }
    }
}

/// Handle `findCallers` LSP query - two-step call hierarchy.
///
/// Step 1: `callHierarchy/prepare` at the cursor position to get a
/// `CallHierarchyItem`. Step 2: `callHierarchy/incomingCalls` on that item
/// to retrieve all caller locations.
async fn find_callers(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::PositionQuery {
        file_path,
        line,
        character,
        ..
    } = input
    else {
        return make_error_result("findCallers", "internal: wrong input variant");
    };
    let op = "findCallers";
    let uri = format!("file://{}", file_path);

    // Step 1: callHierarchy/prepareCallHierarchy
    let prepare_params = serde_json::json!({
        "textDocument": {"uri": uri},
        "position": {"line": line, "character": character}
    });
    let prepare_raw = await_lsp_reply(handle, "callHierarchy/prepare", prepare_params).await;

    let item = match prepare_raw {
        Err(e) => return make_error_result(op, &e.to_string()),
        Ok(v) => {
            let items = v.as_array().and_then(|a| a.first().cloned());
            match items {
                None => {
                    return make_success_result(
                        op,
                        0,
                        "No call hierarchy item found at position".to_string(),
                    )
                }
                Some(i) => i,
            }
        }
    };

    // Step 2: callHierarchy/incomingCalls
    let incoming_params = serde_json::json!({
        "item": item
    });
    let incoming_raw =
        await_lsp_reply(handle, "callHierarchy/incomingCalls", incoming_params).await;

    match incoming_raw {
        Err(e) => make_error_result(op, &e.to_string()),
        Ok(v) => {
            let calls = v.as_array().cloned().unwrap_or_default();
            if calls.is_empty() {
                return make_success_result(op, 0, "No callers found".to_string());
            }
            let locations: Vec<LspLocation> = calls
                .iter()
                .filter_map(|call| {
                    let from = &call["from"];
                    let uri = from["uri"].as_str()?;
                    let start_line = from["range"]["start"]["line"].as_u64()? as u32;
                    let start_char = from["range"]["start"]["character"].as_u64()? as u32;
                    Some(
                        LspLocation::builder()
                            .uri(RootUri::from(uri.to_owned()))
                            .start_line(LineNumber::of(start_line))
                            .start_character(CharacterOffset::of(start_char))
                            .build(),
                    )
                })
                .collect();

            if locations.is_empty() {
                make_success_result(op, 0, "No callers found".to_string())
            } else {
                let count = locations.len();
                let text = format_locations(&locations).await;
                make_success_result(op, count, text.as_str().to_owned())
            }
        }
    }
}

/// Handle `rename` LSP query - semantic rename of a symbol across the workspace.
async fn rename_symbol(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::RenameQuery {
        file_path,
        line,
        character,
        new_name,
    } = input
    else {
        return make_error_result("rename", "internal: wrong input variant");
    };
    let op = "rename";
    let uri = format!("file://{}", file_path);
    let params = serde_json::json!({
        "textDocument": {"uri": uri},
        "position": {"line": line, "character": character},
        "newName": new_name
    });

    let raw = await_lsp_reply(handle, "textDocument/rename", params).await;

    match handle_lsp_response(raw) {
        Err(e) => make_error_result(op, e.as_ref()),
        Ok(v) => {
            // Collect document changes from the WorkspaceEdit result
            let changes = &v["changes"];
            let document_changes = &v["documentChanges"];
            let mut total_edits: usize = 0;

            // Format the changes summary
            let mut summary_lines = Vec::new();
            if let Some(doc_map) = changes.as_object() {
                for (doc_uri, edits) in doc_map {
                    let edits_arr = edits.as_array().map(|a| a.len()).unwrap_or(0);
                    total_edits += edits_arr;
                    let path = doc_uri.strip_prefix("file://").unwrap_or(doc_uri);
                    summary_lines.push(format!("{}: {} edit(s)", path, edits_arr));
                }
            } else if let Some(doc_changes_arr) = document_changes.as_array() {
                for change in doc_changes_arr {
                    if let Some(text_doc_edit) = change.get("textDocument")
                            && let Some(edits) = change.get("edits").and_then(|e| e.as_array())
                        {
                            let doc_uri = text_doc_edit["uri"].as_str().unwrap_or("?");
                            total_edits += edits.len();
                            let path = doc_uri.strip_prefix("file://").unwrap_or(doc_uri);
                            summary_lines.push(format!("{}: {} edit(s)", path, edits.len()));
                        }
                }
            }

            if total_edits == 0 {
                make_success_result(op, 0, "Rename completed: no changes needed".to_string())
            } else {
                let text = format!(
                    "Renamed symbol across {} file(s), {} total edit(s):\n{}",
                    summary_lines.len(),
                    total_edits,
                    summary_lines.join("\n"),
                );
                make_success_result(op, total_edits, text)
            }
        }
    }
} // ── Internal helpers ──────────────────────────────────────────────────────────

/// Reject any path containing `..` components to prevent path traversal attacks.
fn reject_path_traversal(path: &str) -> Result<(), String> {
    let has_traversal = std::path::Path::new(path)
        .components()
        .any(|c| c == std::path::Component::ParentDir);
    if has_traversal {
        return Err(format!(
            "file_path must not contain '..' components: {path}"
        ));
    }
    Ok(())
}

/// Check that a file exists on disk, returning an error string if not.
async fn check_file_exists(path: &str) -> Result<(), String> {
    reject_path_traversal(path)?;
    tokio::fs::metadata(path)
        .await
        .map(|_| ())
        .map_err(|_| format!("file not found: {}", path))
}

/// Dispatch the validated [`LspQueryInput`] to the correct per-operation function.
///
/// # Postconditions
///
/// - Returns a fully populated [`ToolCallResult`] with `session_log: Some(...)`.
async fn dispatch_operation(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    match input {
        LspQueryInput::PositionQuery { operation, .. } => {
            dispatch_position_operation(handle, input, operation).await
        }
        LspQueryInput::FileQuery { .. } => document_symbols(handle, input).await,
        LspQueryInput::SymbolQuery { .. } => workspace_symbols(handle, input).await,
        LspQueryInput::RenameQuery { .. } => rename_symbol(handle, input).await,
    }
}

async fn dispatch_position_operation(
    handle: &dyn LspClient,
    input: &LspQueryInput,
    operation: &LspOperation,
) -> ToolCallResult {
    // Resolve symbol_name to coordinates if provided
    let resolved_input = resolve_symbol_name_if_needed(handle, input).await;

    match operation {
        LspOperation::GoToDefinition => go_to_definition(handle, &resolved_input).await,
        LspOperation::FindReferences => find_references(handle, &resolved_input).await,
        LspOperation::Hover => hover_info(handle, &resolved_input).await,
        LspOperation::GoToImplementation => go_to_implementation(handle, &resolved_input).await,
        LspOperation::FindCallers => find_callers(handle, &resolved_input).await,
        LspOperation::DocumentSymbol => make_error_result(
            "goToImplementation",
            "operation not valid at position context",
        ),
        LspOperation::WorkspaceSymbol => make_error_result(
            "goToImplementation",
            "operation not valid at position context",
        ),
        LspOperation::Rename => make_error_result("rename", "use the rename operation instead"),
    }
}

/// Handle `goToDefinition` LSP query.
async fn go_to_definition(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::PositionQuery {
        file_path,
        line,
        character,
        ..
    } = input
    else {
        return make_error_result("goToDefinition", "internal: wrong input variant");
    };
    let op = "goToDefinition";
    let uri = format!("file://{}", file_path);
    let params = serde_json::json!({
        "textDocument": {"uri": uri},
        "position": {"line": line, "character": character}
    });

    let raw = await_lsp_reply(handle, "textDocument/definition", params).await;

    match handle_lsp_response(raw) {
        Err(e) => make_error_result(op, e.as_ref()),
        Ok(v) => {
            let locations = parse_locations(&v);
            if locations.is_empty() {
                make_success_result(op, 0, "No definition found".to_string())
            } else {
                let count = locations.len();
                let text = format_locations(&locations).await;
                make_success_result(op, count, text.as_str().to_owned())
            }
        }
    }
}

/// Handle `findReferences` LSP query.
async fn find_references(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::PositionQuery {
        file_path,
        line,
        character,
        ..
    } = input
    else {
        return make_error_result("findReferences", "internal: wrong input variant");
    };
    let op = "findReferences";
    let uri = format!("file://{}", file_path);
    let params = serde_json::json!({
        "textDocument": {"uri": uri},
        "position": {"line": line, "character": character},
        "context": {"includeDeclaration": true}
    });

    let raw = await_lsp_reply(handle, "textDocument/references", params).await;

    match handle_lsp_response(raw) {
        Err(e) => make_error_result(op, e.as_ref()),
        Ok(v) => {
            let locations = parse_locations(&v);
            if locations.is_empty() {
                make_success_result(op, 0, "No references found".to_string())
            } else {
                let count = locations.len();
                let text = format_locations(&locations).await;
                make_success_result(op, count, text.as_str().to_owned())
            }
        }
    }
}

/// Handle `hover` LSP query.
async fn hover_info(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::PositionQuery {
        file_path,
        line,
        character,
        ..
    } = input
    else {
        return make_error_result("hover", "internal: wrong input variant");
    };
    let op = "hover";
    let uri = format!("file://{}", file_path);
    let params = serde_json::json!({
        "textDocument": {"uri": uri},
        "position": {"line": line, "character": character}
    });

    let raw = await_lsp_reply(handle, "textDocument/hover", params).await;

    match handle_lsp_response(raw) {
        Err(error) => make_error_result(op, error.as_ref()),
        Ok(value) => build_hover_result(op, &value),
    }
}

fn build_hover_result(op: &str, value: &serde_json::Value) -> ToolCallResult {
    if value.is_null() {
        return make_success_result(op, 0, "No hover information found".to_string());
    }
    match extract_hover_text(value) {
        Some(text) => make_success_result(op, 1, text),
        None => make_success_result(op, 0, "No hover information found".to_string()),
    }
}

fn extract_hover_text(value: &serde_json::Value) -> Option<String> {
    let contents = &value["contents"];
    contents
        .as_str()
        .map(str::to_owned)
        .or_else(|| contents["value"].as_str().map(str::to_owned))
}

/// Handle `documentSymbol` LSP query.
async fn document_symbols(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::FileQuery { file_path } = input else {
        return make_error_result("documentSymbol", "internal: wrong input variant");
    };
    let op = "documentSymbol";
    let uri = format!("file://{}", file_path);
    let params = serde_json::json!({
        "textDocument": {"uri": uri}
    });

    let raw = await_lsp_reply(handle, "textDocument/documentSymbol", params).await;

    match handle_lsp_response(raw) {
        Err(e) => make_error_result(op, e.as_ref()),
        Ok(v) => {
            let symbols = flatten_document_symbols(&v);
            if symbols.is_empty() {
                make_success_result(op, 0, "No symbols found".to_string())
            } else {
                let count = symbols.len();
                let text = format_symbols(&symbols);
                make_success_result(op, count, text.as_str().to_owned())
            }
        }
    }
}

/// Handle `workspaceSymbol` LSP query.
async fn workspace_symbols(handle: &dyn LspClient, input: &LspQueryInput) -> ToolCallResult {
    let LspQueryInput::SymbolQuery { query } = input else {
        return make_error_result("workspaceSymbol", "internal: wrong input variant");
    };
    let op = "workspaceSymbol";
    let params = serde_json::json!({ "query": query });

    let raw = await_lsp_reply(handle, "workspace/symbol", params).await;

    match handle_lsp_response(raw) {
        Err(e) => make_error_result(op, e.as_ref()),
        Ok(v) => {
            let symbols = flatten_document_symbols(&v);
            if symbols.is_empty() {
                make_success_result(op, 0, "No workspace symbols found".to_string())
            } else {
                let count = symbols.len();
                let text = format_symbols(&symbols);
                make_success_result(op, count, text.as_str().to_owned())
            }
        }
    }
}

/// Recursively flatten a single symbol item into the accumulator.
fn flatten_symbol(item: &serde_json::Value, default_uri: &str, acc: &mut Vec<LspSymbol>) {
    let name = item["name"].as_str().unwrap_or("").to_owned();
    let kind_num = item["kind"].as_u64().unwrap_or(0) as u32;
    let kind = symbol_kind_name(kind_num).to_owned();

    if item["selectionRange"].is_object() {
        // DocumentSymbol format
        let start_line = item["selectionRange"]["start"]["line"]
            .as_u64()
            .unwrap_or(0) as u32;
        acc.push(
            LspSymbol::builder()
                .name(name)
                .kind(kind)
                .uri(RootUri::from(default_uri.to_owned()))
                .start_line(LineNumber::of(start_line))
                .build(),
        );
        // Recurse into children (depth-first pre-order)
        if let Some(children) = item["children"].as_array() {
            for child in children {
                flatten_symbol(child, default_uri, acc);
            }
        }
    } else if item["location"].is_object() {
        // SymbolInformation format
        let uri = item["location"]["uri"]
            .as_str()
            .unwrap_or(default_uri)
            .to_owned();
        let start_line = item["location"]["range"]["start"]["line"]
            .as_u64()
            .unwrap_or(0) as u32;
        acc.push(
            LspSymbol::builder()
                .name(name)
                .kind(kind)
                .uri(RootUri::from(uri))
                .start_line(LineNumber::of(start_line))
                .build(),
        );
    }
}

/// Map an LSP `SymbolKind` number to its name string.
///
/// Returns `"Unknown"` for values outside the defined range `1..=26`.
fn symbol_kind_name(kind: u32) -> &'static str {
    SYMBOL_KIND_NAMES
        .get(kind as usize)
        .copied()
        .unwrap_or("Unknown")
}

/// Parse an LSP location value (null, single object, or array) into a `Vec<LspLocation>`.
fn parse_locations(value: &serde_json::Value) -> Vec<LspLocation> {
    if value.is_null() {
        return Vec::new();
    }
    if let Some(arr) = value.as_array() {
        return arr.iter().filter_map(parse_single_location).collect();
    }
    // Single location object
    parse_single_location(value).into_iter().collect()
}

/// Parse a single JSON location object into `LspLocation`, or `None` if malformed.
fn parse_single_location(v: &serde_json::Value) -> Option<LspLocation> {
    let uri = v["uri"].as_str()?.to_owned();
    let start_line = v["range"]["start"]["line"].as_u64()? as u32;
    let start_char = v["range"]["start"]["character"].as_u64()? as u32;
    Some(
        LspLocation::builder()
            .uri(RootUri::from(uri))
            .start_line(LineNumber::of(start_line))
            .start_character(CharacterOffset::of(start_char))
            .build(),
    )
}

/// Read a single source line from a file URI, trimmed and truncated to 120 chars.
///
/// - Strips `"file://"` prefix if present.
/// - Returns `None` if the file is unreadable or `line_idx` is out of range.
/// - Lines exceeding 120 Unicode scalar values are truncated to 120 and
///   `U+2026 HORIZONTAL ELLIPSIS` is appended.
async fn read_snippet(uri: &str, line_idx: usize) -> Option<String> {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    let content = tokio::fs::read_to_string(path).await.ok()?;
    let line = content.lines().nth(line_idx)?;
    let trimmed = line.trim_start();
    let char_count = trimmed.chars().count();
    if char_count > SNIPPET_MAX_CHARS {
        let truncated: String = trimmed.chars().take(SNIPPET_MAX_CHARS).collect();
        Some(format!("{}\u{2026}", truncated))
    } else {
        Some(trimmed.to_owned())
    }
}

/// Send `method` + `params` to the LSP actor handle and await the reply, with a
/// 10-second timeout.
///
/// On timeout, returns `Err(LspError::RequestTimeout)`.
async fn await_lsp_reply(
    handle: &dyn LspClient,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, LspError> {
    match tokio::time::timeout(
        Duration::from_secs(LSP_REQUEST_TIMEOUT_SECS),
        handle.request(method.to_owned(), params),
    )
    .await
    {
        Err(_elapsed) => Err(LspError::RequestTimeout),
        Ok(result) => result,
    }
}

/// Build an `is_error: false` `ToolCallResult` with a session log.
fn make_success_result(op: &str, count: usize, text: String) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(text))
        .is_error(IsPredicate::from(false))
        .session_log(make_session_log(
            &OutputText::new(op.to_owned()),
            Some(Count::from(count)),
        ))
        .build()
}

/// Build an `is_error: true` `ToolCallResult` with a session log.
fn make_error_result(op: &str, msg: &str) -> ToolCallResult {
    ToolCallResult::builder()
        .name(ToolName::new(TOOL_NAME))
        .output(OutputText::new(msg))
        .is_error(IsPredicate::from(true))
        .session_log(make_session_log(&OutputText::new(op.to_owned()), None))
        .build()
}

#[cfg(test)]
#[path = "../../../tests/tools/builtin/lsp_query.tests.rs"]
mod tests;
