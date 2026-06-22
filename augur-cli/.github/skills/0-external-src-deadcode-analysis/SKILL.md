---
name: 0-external-src-deadcode-analysis
description: >
  Src-only deadcode analyzer that builds a symbol reachability graph from crate
  entrypoints and reports unreachable symbols as true dead code. Reporting only;
  no code changes.
---

# run.sh

## When to use

Use this skill when you need deterministic, read-only deadcode findings limited
to a repository-relative Rust path.

## Key Files

- `run.sh` - Canonical wrapper for src deadcode analysis

## Scope

- Analyzes Rust source under a repository-relative Rust path.
- Builds symbol-level reachability from entrypoints (`main` and public `lib` API roots).
- Reports `true_dead_code` for symbols unreachable from the entrypoint root set.
- Private functions are only reported when they have no inbound references at all,
  which suppresses internal helper chains that are still used within the file.
- Does not apply fixes, rewrites, or deletions.

## Run

```bash
.github/skills/0-external-src-deadcode-analysis/run.sh [<repo-relative-rust-path>] [--format <format>]
```

## Arguments

- `[<repo-relative-rust-path>]` - Repository-relative Rust path to analyze (default: repository Rust source root)
- `--format <format>` - Output format: `text` | `json` (default: `text`)

## Examples

```bash
# Analyze the default repository Rust path with text output
.github/skills/0-external-src-deadcode-analysis/run.sh

# Analyze a specific Rust path and emit JSON
.github/skills/0-external-src-deadcode-analysis/run.sh <repo-relative-rust-path> --format json
```

## Determinism and safety

- Read-only reporting workflow.
- Input scope is explicit and repository-relative.
- Findings include evidence: `reference_count`, `referenced_files`, and `is_public`.
- Exit codes: `0` when clean, `1` when unreachable symbols exist, `2` on runtime/config errors.
