---
name: lsp-query-usage
description: >
  Complete usage reference for the lsp_query tool. Covers coordinate system
  rules, per-operation parameter requirements, recommended workflows, and
  error handling. Read before performing any multi-step code navigation.
---

# Skill: lsp-query-usage

## The lsp_query Tool

`lsp_query` routes queries to a running rust-analyzer instance. It supports
8 operations. All operations require the `operation` field. Other fields are
required or optional depending on the operation.

## Coordinate System: Critical Rule

**Input coordinates are zero-based.** The `line` and `character` fields sent
to `lsp_query` must be zero-based (the first line of a file is line 0, the
first character of a line is character 0).

**Output coordinates are displayed as one-based.** The tool adds 1 to all
line and character values before displaying results, matching the convention
editors use for display.

**Round-trip rule:** When a subsequent `lsp_query` call must use coordinates
that appeared in a previous `lsp_query` result, subtract 1 from both values
before using them as input.

Example: if `lsp_query` returns "defined at line 42, character 5", the input
for a follow-up call targeting that position is `line: 41, character: 4`.

Violating this rule causes the follow-up call to target the wrong position,
producing either a wrong result or a "no symbol at position" error.

## Operations and Parameter Requirements

### goToDefinition

Jumps to where a symbol is defined.

Required: `operation`, and either (`file_path` + `line` + `character`) or
(`file_path` + `symbol_name`).

Optional: `symbol_name` as an alternative to `line` + `character` when the
symbol name is unambiguous in the file. If multiple symbols share the name
within the file, rust-analyzer may return results for the wrong one; prefer
explicit coordinates when available.

Returns: definition location(s) with file path, line, and character (displayed
as one-based).

### findReferences

Lists all sites where a symbol is used across the workspace.

Required: `operation`, `file_path`, and either (`line` + `character`) or
`symbol_name`.

Returns: list of reference locations, each with file path, line, and character.
Results include the definition site itself unless rust-analyzer filters it.

### hover

Returns type information and documentation for the symbol at a position.

Required: `operation`, `file_path`, and either (`line` + `character`) or
`symbol_name`.

Returns: hover text containing the type signature and any doc comment for the
symbol.

### documentSymbol

Lists all symbols declared in a single file.

Required: `operation`, `file_path`.

Does not require coordinates. Returns a list of symbol names, kinds
(function, struct, enum, trait, etc.), and their line ranges within the file.

Use this operation to find coordinates for a named symbol when you know its
file but not its position. Read the returned line numbers (remembering they are
one-based in the output), then subtract 1 when using them as input for a
subsequent position-based operation.

### workspaceSymbol

Searches for symbols matching a query string across the entire workspace.

Required: `operation`, `query` (the search string).

Does not require `file_path` or coordinates. Returns a list of matching symbols
with their file paths and positions (one-based in the output).

Use this as the first step when you know a symbol name but not its file. Follow
up with `goToDefinition` or `hover` using the returned location, remembering to
apply the round-trip coordinate correction.

### goToImplementation

Finds all concrete implementations of a trait or trait method at a given
position. Used to find which structs implement a given trait.

Required: `operation`, `file_path`, and either (`line` + `character`) or
`symbol_name`. Position or name must resolve to a trait definition or a trait
method declaration.

Returns: list of implementation sites.

### findCallers

Finds all call sites for a function or method using the LSP call hierarchy
protocol. This operation is internally a two-step LSP exchange (prepare call
hierarchy, then incoming calls); both steps are handled inside the tool and the
agent receives the final result.

Required: `operation`, `file_path`, and either (`line` + `character`) or
`symbol_name`. Position or name must resolve to a function or method definition.

Returns: list of caller locations with file path, line, and character.

### rename

Semantically renames a symbol across the entire workspace. This operation
produces a workspace edit; the tool applies it and reports which files were
modified.

Required: `operation`, `file_path`, `new_name`, and either
(`line` + `character`) or `symbol_name`.

Returns: list of files modified and the number of substitutions made.

Use with care. Verify with `findReferences` first to understand the scope of
the rename before committing to it.

## Recommended Workflows

### When you know the symbol name but not its location

1. Call `workspaceSymbol` with the symbol name as the `query`.
2. Identify the correct match from the results (file path and position).
3. Subtract 1 from the returned line and character values.
4. Use those corrected coordinates in the follow-up operation.

### When you know the file but not the position

1. Call `documentSymbol` with the `file_path`.
2. Find the symbol in the results list and read its line number.
3. Subtract 1 from the line number.
4. Use that corrected coordinate as `line` in the follow-up operation.
   Set `character: 0` as a starting point; rust-analyzer will resolve the
   symbol at that line even if the character offset is not exact for most
   symbol kinds.

### When verifying all callers before modifying a function

1. Call `findCallers` on the function to enumerate all call sites.
2. Read each call site path and position.
3. For each call site, call `hover` to confirm the call signature matches
   what you expect before making changes.

### When finding all implementations of a trait

1. Call `workspaceSymbol` with the trait name.
2. Use the returned position (corrected for round-trip) to call
   `goToImplementation`.
3. Each returned implementation site is a struct that implements the trait.

## Error Handling

**"no symbol at position"** - The coordinates do not point to a recognized
symbol. Check whether you applied the round-trip correction (subtract 1 from
one-based display values before submitting as input).

**"rust-analyzer not found"** - rust-analyzer is not on PATH.
Install with: `rustup component add rust-analyzer`

**"request timed out"** - The tool waited 30 seconds and received no response.
This can happen during initial workspace indexing. Wait briefly and retry.

**"process exited unexpectedly"** - The rust-analyzer child process crashed.
The LspActor will attempt to surface this. Retry; if it persists, the rust-analyzer
binary may need reinstalling.

**"ambiguous symbol name"** - When using `symbol_name` and the name matches
multiple symbols. Use coordinates instead, or narrow the `file_path` to the
file containing the specific symbol you want.

## When Not to Use lsp_query

Use shell-based search (`shell_exec` with `grep`, `rg`, or `fd`) when:
- You need literal text matches, including in comments or string literals
- You are searching by filename pattern
- The symbol name is not yet defined (e.g. checking whether a name is
  already taken before creating it)
- You need to find all occurrences of a string that is not a code symbol
  (e.g. a configuration key or log message)

The rule: use `lsp_query` for semantic symbol navigation; use text search for
everything else.