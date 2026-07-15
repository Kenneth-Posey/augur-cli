---
name: 0-external-doc-extractor
description: >
  Extract public Rust items into summary, index, full, or missing-docs output.
  Use `--full-input` with `--tier full` when the source path is ambiguous.
---

# doc-extractor

## When to use

Use this skill when you need to extract public Rust API surfaces, generate
documentation indexes, retrieve full doc comments, or identify undocumented
public items for review or coverage analysis.

## Purpose

Deterministic documentation extractor that renders public Rust items into
summary, index, full, or missing-docs output. Supports both source-file and
rustdoc JSON input for the full-doc tier. Source modifications in
`.github/skills/0-external-doc-extractor/` require a local build via
`cd .github/skills/0-external-doc-extractor && cargo build --release`.

## Run

```bash
.github/skills/0-external-doc-extractor/run.sh <source-path> [--tier <tier>] [--module <name>] [--full-input <mode>]
```

The skill directory also provides tier-specific wrappers so callers do not need
to remember the `--tier` flag:

| Wrapper | Equivalent |
|---------|-----------|
| `.github/skills/0-external-doc-extractor/run-summary.sh` | `run.sh <path> --tier summary` |
| `.github/skills/0-external-doc-extractor/run-index.sh` | `run.sh <path> --tier index` |
| `.github/skills/0-external-doc-extractor/run-full.sh` | `run.sh <path> --tier full --module <name> --full-input source` |
| `.github/skills/0-external-doc-extractor/run.sh` | General-purpose wrapper accepting all flags |

Use the tier-specific wrappers when the tier is fixed and known ahead of time.

## Arguments

| Argument | Description |
|----------|-------------|
| `<SOURCE>` | Path to the extractor input file or directory. Required. |
| `--tier <TIER>` | Output tier to render. Values: `summary`, `index`, `full`, `missing-docs`. Default: `summary`. |
| `--module <MODULE>` | Module name to use for the full-doc tier. Defaults to the file stem of the source path. Optional. |
| `--full-input <FULL_INPUT>` | Explicit full-doc input mode for `--tier full`. Values: `source` (read from Rust source files or directories), `rustdoc` (read from rustdoc-backed JSON input). Required when the source path does not clearly indicate Rust source. |
| `-h, --help` | Print help. |

When using rustdoc JSON input for full-tier extraction, do not read the JSON
file directly in the caller; pass its path to `run.sh` and let the tool
consume it.

## Output

Each tier produces a distinct output format:

- **summary** - Compact plain-text listing of public items with one-line descriptions.
- **index** - Plain-text item index for navigation and quick-scan.
- **full** - Full per-module documentation including complete doc texts.
- **missing-docs** - JSON report of public items that have no doc comment.

Exit codes: `0` on success, non-zero on error.

## Examples

```bash
# Extract summary of all public items
.github/skills/0-external-doc-extractor/run.sh src/lib.rs

# Same with tier-specific wrapper
.github/skills/0-external-doc-extractor/run-summary.sh src/lib.rs

# Extract an index for navigation
.github/skills/0-external-doc-extractor/run-index.sh src/lib.rs

# Extract full documentation for a module
.github/skills/0-external-doc-extractor/run-full.sh src/lib.rs --module my_module

# Alternative: general wrapper with explicit flags
.github/skills/0-external-doc-extractor/run.sh src/lib.rs --tier full --module my_module --full-input source

# Find undocumented public items
.github/skills/0-external-doc-extractor/run.sh src/lib.rs --tier missing-docs
```

## Key Files

- `.github/skills/0-external-doc-extractor/SKILL.md` - This skill definition
- `.github/skills/0-external-doc-extractor/run.sh` - General-purpose wrapper
- `.github/skills/0-external-doc-extractor/run-summary.sh` - Summary-tier wrapper
- `.github/skills/0-external-doc-extractor/run-index.sh` - Index-tier wrapper
- `.github/skills/0-external-doc-extractor/run-full.sh` - Full-doc wrapper
- `.github/skills/0-external-doc-extractor/doc-extractor` - Compiled tool binary
