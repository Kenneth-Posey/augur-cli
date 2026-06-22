---
name: 0-external-stub-detector
description: >
  Stub analyzer that detects deferred patterns (`todo!()`, `unimplemented!()`, `panic!()`,
  `unwrap()`, `expect()`) in Rust source code. Reporting only; no code changes.
---

# run.sh

## When to use

Use this skill when you need deterministic, read-only stub detection limited
to a repository-relative Rust path.

## Scope

- Analyzes Rust source under a repository-relative Rust path.
- Detects deferred patterns: `todo!()`, `unimplemented!()`, `panic!()`, `unwrap()`, `expect()`.
- Reports findings with severity classification and location information.
- Does not apply fixes, rewrites, or deletions.

## Run

```bash
.github/skills/0-external-stub-detector/run.sh [<repo-relative-rust-path>] [--format <format>]
```

## Arguments

- `[<repo-relative-rust-path>]` - Repository-relative Rust path to analyze (default: repository Rust source root)
- `--format <format>` - Output format: `text` | `json` (default: `text`)

## Examples

```bash
# Analyze the default repository Rust path with text output
.github/skills/0-external-stub-detector/run.sh

# Analyze a specific Rust path and emit JSON
.github/skills/0-external-stub-detector/run.sh <repo-relative-rust-path> --format json

# Analyze a specific path with JSON output
.github/skills/0-external-stub-detector/run.sh <repo-relative-rust-path> --format json
```

## Determinism and safety

- Read-only reporting workflow.
- Input scope is explicit and repository-relative.
- Findings include evidence: file path, line number, column, pattern type, and severity.
- Exit codes: `0` when clean, `1` when deferred patterns exist, `2` on runtime/config errors.

## Output contract

When `--format json` is specified, output is valid JSON with the following schema:

```json
{
  "findings": [
    {
      "file": "<repo-relative-rust-file>",
      "line": 42,
      "column": 8,
      "pattern": "todo",
      "severity": "high",
      "context": "function body"
    }
  ],
  "summary": {
    "total": 1,
    "by_pattern": {
      "todo": 1
    }
  }
}
```

Pattern severity levels:
- `todo`, `unimplemented`: **high** (definite deferred behavior)
- `panic`: **medium** (can be legitimate in error paths; context-dependent)
- `unwrap`, `expect`: **low** (runtime error risk; requires manual judgment)

## Key Files

- `run.sh` - Canonical wrapper for stub detector