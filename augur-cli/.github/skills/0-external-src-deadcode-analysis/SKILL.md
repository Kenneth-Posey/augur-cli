---
name: 0-external-src-deadcode-analysis
description: >
  Src-only deadcode analyzer that builds a symbol reachability graph from crate
  entrypoints and reports unreachable symbols as true dead code. Reporting only;
  no code changes.
---

# src-deadcode-analysis

## When to use

Use this skill when you need deterministic, read-only deadcode findings limited
to a repository-relative Rust path.

## Purpose

Detect top-level symbols under `src/` that are not referenced by other source
files. Builds a symbol-level reachability graph from crate entrypoints (`main`
and public `lib` API roots) and reports symbols unreachable from that root set.
Private functions are only reported when they have no inbound references at all,
which suppresses internal helper chains that are still used within the file.
Does not apply fixes, rewrites, or deletions. Input scope is explicit and
repository-relative.

## Run

```bash
.github/skills/0-external-src-deadcode-analysis/run.sh [<TARGET_PATH>] [OPTIONS]
```

## Arguments

- `<TARGET_PATH>` - Directory to analyze (default: `src`)
- `-f, --format <FORMAT>` - Output format: `text` | `json` (default: `text`)
- `-h, --help` - Print help
- `-V, --version` - Print version

## Output

Produces a symbol deadcode report listing symbols unreachable from the
entrypoint root set. Available output formats:

- **text** (default): Human-readable listing of unreachable symbols.
- **json**: Machine-readable output with per-symbol evidence including
  `reference_count`, `referenced_files`, and `is_public`.

Exit codes:

- `0` - No unreachable symbols detected (clean).
- `1` - Unreachable symbols exist.
- `2` - Runtime or configuration error.

## Examples

```bash
# Analyze the default src/ directory with text output
.github/skills/0-external-src-deadcode-analysis/run.sh

# Analyze a specific path and emit JSON
.github/skills/0-external-src-deadcode-analysis/run.sh src/domain --format json
```

## Key Files

- `.github/skills/0-external-src-deadcode-analysis/run.sh` - Canonical wrapper for src deadcode analysis