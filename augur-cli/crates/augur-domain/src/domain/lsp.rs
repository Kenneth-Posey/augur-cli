//! Domain types for the LSP query tool.
//!
//! Covers the eight query operations the LLM may request, the error modes
//! surfaced outside the actor, the validated input representation, and the
//! two value-object result types (`LspLocation`, `LspSymbol`).
//!
//! **Coordinate convention:** `start_line` and `start_character` fields carry
//! 0-based LSP wire values. Callers that display coordinates to the LLM must
//! add `+ 1` before formatting.

use crate::domain::newtypes::{CharacterOffset, LineNumber};
use crate::domain::string_newtypes::RootUri;

/// The eight LSP actions the LLM may request via the `lsp_query` tool.
///
/// Parsed from the raw `"operation"` string in tool arguments. An unrecognised
/// string produces a validation error and is never stored as a variant.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LspOperation {
    /// `textDocument/definition` - jump to where a symbol is defined.
    GoToDefinition,
    /// `textDocument/references` - list all reference sites of a symbol.
    FindReferences,
    /// `textDocument/hover` - retrieve hover documentation for a position.
    Hover,
    /// `textDocument/documentSymbol` - list symbols declared in a single file.
    DocumentSymbol,
    /// `workspace/symbol` - search symbols across the whole workspace.
    WorkspaceSymbol,
    /// `textDocument/implementation` - find all concrete implementations of
    /// a trait or trait method at the given position.
    GoToImplementation,
    /// `callHierarchy/incomingCalls` - two-step operation that finds all
    /// callers of a function or method at the given position.
    FindCallers,
    /// `textDocument/rename` - semantically rename a symbol across the
    /// workspace, understanding scope so it avoids false matches.
    Rename,
}

/// Every failure mode that can be observed outside the `LspActor`.
///
/// `RequestTimeout` is constructed **only** in the tool layer after a
/// `tokio::time::timeout` fires; the actor itself never produces it.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LspError {
    /// rust-analyzer binary was not found on `PATH`.
    #[error("rust-analyzer not found; install it with: rustup component add rust-analyzer")]
    NotInstalled,

    /// The LSP initialize / initialized handshake did not complete.
    #[error("rust-analyzer initialization failed: {detail}")]
    InitFailed {
        /// Human-readable description of why initialization failed.
        detail: String,
    },

    /// The tool layer did not receive a response within its deadline.
    ///
    /// Constructed **only** in the tool layer (`src/tools/builtin/lsp_query.rs`).
    /// The actor never emits this variant.
    #[error("lsp request timed out after 10s")]
    RequestTimeout,

    /// The rust-analyzer child process exited unexpectedly.
    #[error("rust-analyzer process exited unexpectedly")]
    ProcessDied,

    /// A JSON-RPC framing or parsing error occurred.
    #[error("{0}")]
    Protocol(String),
}

/// Validated, typed representation of the tool arguments after `validate_args`
/// succeeds.
///
/// Four variants cover all LSP operations:
/// - `PositionQuery` - operations that need a file + cursor position
///   (`GoToDefinition`, `FindReferences`, `Hover`, `GoToImplementation`, `FindCallers`).
/// - `FileQuery` - `DocumentSymbol` scoped to one file (operation is implicit).
/// - `SymbolQuery` - `WorkspaceSymbol` across the workspace (operation is implicit).
/// - `RenameQuery` - `Rename` scoped to a position (operation is implicit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspQueryInput {
    /// A query anchored to a specific cursor position within a file.
    PositionQuery {
        /// Which LSP operation to perform at this position.
        operation: LspOperation,
        /// Absolute or workspace-relative path to the source file.
        file_path: String,
        /// 0-based line index (LSP wire value).
        line: u32,
        /// 0-based character offset (LSP wire value).
        character: u32,
        /// Optional symbol name to resolve via `workspace/symbol` when
        /// exact `line`/`character` coordinates are not known. If provided
        /// and `line`/`character` are omitted, the tool resolves the
        /// name internally to determine coordinates.
        symbol_name: Option<String>,
    },

    /// A query scoped to an entire file without a specific cursor position.
    /// The operation is always `DocumentSymbol` - implicit in the variant identity.
    FileQuery {
        /// Absolute or workspace-relative path to the source file.
        file_path: String,
    },

    /// A rename request requiring the new name to apply.
    /// The operation is always `Rename` - implicit in the variant identity.
    RenameQuery {
        /// Absolute or workspace-relative path to the source file.
        file_path: String,
        /// 0-based line index (LSP wire value).
        line: u32,
        /// 0-based character offset (LSP wire value).
        character: u32,
        /// The new name to apply to the symbol.
        new_name: String,
    },

    /// A workspace-wide symbol search driven by a name query string.
    /// The operation is always `WorkspaceSymbol` - implicit in the variant identity.
    SymbolQuery {
        /// The symbol name (or prefix) to search for across the workspace.
        query: String,
    },
}

/// A single file-position result returned by definition or reference operations.
///
/// `start_line` and `start_character` carry 0-based LSP wire values; add `+ 1`
/// before displaying to the LLM.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, bon::Builder)]
pub struct LspLocation {
    /// URI of the file containing this location, as returned by the LSP server.
    pub uri: RootUri,
    /// 0-based line index (LSP wire value; add `+ 1` for display).
    pub start_line: LineNumber,
    /// 0-based character offset (LSP wire value; add `+ 1` for display).
    pub start_character: CharacterOffset,
}

/// A named code symbol returned by document-symbol or workspace-symbol operations.
///
/// `start_line` carries a 0-based LSP wire value; add `+ 1` before displaying
/// to the LLM.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, bon::Builder)]
pub struct LspSymbol {
    /// The symbol's identifier as it appears in source code.
    pub name: String,
    /// LSP `SymbolKind` label (e.g. `"Function"`, `"Struct"`, `"Method"`).
    pub kind: String,
    /// URI of the file that declares this symbol, as returned by the LSP server.
    pub uri: RootUri,
    /// 0-based line index of the symbol's declaration (LSP wire value; add `+ 1` for display).
    pub start_line: LineNumber,
}
