---
name: 0-utility-codebase-survey
description: >
  Systematic process for mapping existing code before implementing or
  refactoring. Use before writing any Rust code that integrates with existing
  modules, to prevent duplicating helpers or violating dependency direction.
---

# Codebase Survey Process

All 9 steps are required before writing any implementation code.
Do not skip or reorder steps.

## Step 1: Read Directory Structure

Read [`.github/local/directories.md`](../../local/directories.md) for the repository-relative source tree layout,
test organization, and directory conventions.

## Key Files

- `README.md` - overview and usage notes

## Step 2: Read Architecture Reference

Read `docs/architecture.md` for the module map, actor subsystem boundaries,
dependency direction rules, and execution flow.

## Step 3: Enumerate All Source Files

Using paths from `.github/local/directories.md`, list all current source files:
- `<repo-relative-rust-path>/**/*.rs` - all production source files
- `<repo-relative-test-path>/**/*.rs` - all test files

Record the full list. Do not assume the tree still matches `docs/structure.md`.

## Step 4: Search for Related Symbols

If code intelligence tools are available (LSP symbol lookup, semantic search,
call graphs), prefer them over grep for symbol definitions, call sites, and
type relationships.

Before using grep, check whether doc-extractor has already generated artifacts
for the target path. Start with them when available:
- Summary artifact for one-line descriptions of public items.
- Index artifact for item names and kinds.
- Full-doc artifact for complete per-module documentation.
- `run.sh --tier missing-docs` to find undocumented public items.

Use grep when doc-extractor artifacts are unavailable or when the task needs
symbol-level precision they do not provide.

Search for types, functions, traits, and constants related to the task target.
Use grep on:
- The type or function name you plan to create or modify.
- Any domain concept the task touches (e.g., `Price`, `SessionId`, `ToolHandler`).
- The module path where the new code would live.

Record all matches with file paths.

## Step 5: Read Related Symbol Definitions

For each related symbol found in Step 4, read the containing file section.
Capture:
- Its full interface (signature, parameters, return type, trait bounds).
- Its ownership and lifetime semantics.
- All existing callers or consumers.

## Step 6: Identify Reuse Candidates

Compare the task needs to the symbols found in Steps 4 and 5.
Document:
- Existing helpers that overlap with what the task needs.
- Existing constants that the task should use instead of literals.
- Existing traits that the task should implement or extend.

Do not proceed until all reuse candidates are documented.
Do not reimplement existing helpers.

## Step 7: Map the Dependency Graph

For the target module:
- List all modules it currently imports (`use` statements).
- List all modules that currently import it.
- Confirm that adding the new code or modifying existing code does not create
  a cycle or reverse the allowed dependency direction per `docs/architecture.md`.

## Step 8: Identify the Correct Module Path

Using `docs/structure.md` and the dependency graph from Step 7, determine:
- The exact file path for any new code.
- Whether new code belongs in an existing file or a new module.
- Whether a new supporting module (`ops.rs`, `assistant/`) is the right location.

## Step 9: Begin Implementation

Only after steps 1 through 8 are complete:
- Write failing tests first (TDD Red phase).
- Implement the minimal code to pass tests (TDD Green phase).
- Refactor for clarity without behavior change (TDD Refactor phase).
- Use identified reuse candidates. Do not duplicate existing helpers.
- Place new code at the path determined in Step 8.
