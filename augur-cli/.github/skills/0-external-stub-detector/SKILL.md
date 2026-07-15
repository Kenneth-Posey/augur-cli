---
name: 0-external-stub-detector
description: >
  Stub analyzer that detects deferred patterns (`todo!()`, `unimplemented!()`, `panic!()`,
  `unwrap()`, `expect()`) in Rust source code. Reporting only; no code changes.
---

# stub-detector

## When to use

Use this skill when you need deterministic, read-only stub detection limited to a repository-relative Rust path. Analyzes Rust source for deferred patterns and reports findings with severity classification and location information. Does not apply fixes, rewrites, or deletions.

## Purpose

Stub-detector identifies deferred implementation patterns (`todo!()`, `unimplemented!()`, `panic!()`, `unwrap()`, `expect()`) in Rust source files under a specified repository path. Input scope is explicit and repository-relative. Findings include evidence (file path, line number, column, pattern type, and severity). The tool is read-only and does not modify source code.

## Run

```bash
.github/skills/0-external-stub-detector/run.sh [TARGET_PATH] [--format <FORMAT>]
```

## Arguments

- `[TARGET_PATH]` — File or directory to analyze. Repository-relative Rust path. Default: `src`.
- `-f, --format <FORMAT>` — Output format. Possible values:
  - `text` — Human-readable text format (default)
  - `json` — Machine-readable JSON format
- `-h, --help` — Print help
- `-V, --version` — Print version

## Output

Produces a report of all deferred patterns found. Exit codes:
- `0` — No deferred patterns found (clean)
- `1` — Deferred patterns exist
- `2` — Runtime or configuration error

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
- `todo`, `unimplemented` — **high** (definite deferred behavior)
- `panic` — **medium** (can be legitimate in error paths; context-dependent)
- `unwrap`, `expect` — **low** (runtime error risk; requires manual judgment)

## Examples

```bash
# Analyze the default Rust source path (`src`) with text output
.github/skills/0-external-stub-detector/run.sh

# Analyze a specific file or directory and emit JSON
.github/skills/0-external-stub-detector/run.sh src/domain --format json

# Analyze a single file
.github/skills/0-external-stub-detector/run.sh src/main.rs --format text
```

## Key Files

- `.github/skills/0-external-stub-detector/run.sh` — Canonical wrapper for stub detector