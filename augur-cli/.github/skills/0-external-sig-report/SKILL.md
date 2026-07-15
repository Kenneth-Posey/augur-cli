---
name: 0-external-sig-report
description: >
  Consolidation signal analyzer that loads rustdoc JSON and runs minimal
  signature reports by default, with broader presets for consolidation and
  full-report review handoffs.
---

# sig-report

## When to use

Use this skill during architecture and API review when you need signature-level
evidence from rustdoc JSON, including duplicate function detection, group-size
analysis, and consolidation candidates.

## Purpose

Analyze rustdoc JSON for signature-review evidence. The minimal preset
(`--function-signatures`) is the default. Use `--consolidation` for broader
duplicate-signature and refactoring evidence, or `--all-reports` for every
report family.

## Run

```bash
.github/skills/0-external-sig-report/run.sh [JSON_FILE] [options]
```

## Arguments

**Positional argument:**

`[JSON_FILE]`
: Path to the rustdoc JSON file. Alternatively, pass `--snapshot provided:<path>`,
  `--snapshot cached:<path>`, or `--snapshot generated` to control the snapshot
  source explicitly.

**Report presets (mutually exclusive, overridden by `--reports`):**

- `--function-signatures` -- Use the function-signature review preset (the minimal default)
- `--consolidation` -- Use the broader API-consolidation review preset
- `--all-reports` -- Use every JSON-capable report family
- `--reports <REPORTS>` -- Comma-separated list of reports to run (A-H). Overrides the presets above

**Snapshot control:**

- `--snapshot <SNAPSHOT>` -- Explicit snapshot source: `generated`, `provided:<path>`, or `cached:<path>`. When set, overrides the positional `JSON_FILE` argument.
- `--snapshot-output <SNAPSHOT_OUTPUT>` -- Output path for a generated rustdoc JSON snapshot. Used with `--snapshot generated`.

**Other options:**

- `--min-sig <MIN_SIG>` -- Override the minimum signature-group size (default: 3)
- `--no-color` -- Disable ANSI color output
- `--debug` -- Enable debug-level tracing output
- `-h`, `--help` -- Print help (see a summary with `-h`)

## Output

The tool produces human-readable or machine-readable output depending on the
`--output-format` flag:

- `text` (default) -- Human-readable text output. Findings are printed to stdout
  with ANSI color by default. Exit code is 0 on success, non-zero on error.
- `json` -- Stable JSON output for downstream tools. Emitted to stdout as a
  single JSON object. Exit code is 0 on success, non-zero on error.

When `--snapshot generated` is used, a rustdoc JSON snapshot is written to the
path specified by `--snapshot-output` (or a default location) before the report
is produced.

## Examples

```bash
# Minimal signature review
.github/skills/0-external-sig-report/run.sh <myapp>.json \
  --function-signatures \
  --output-format json

# Broader consolidation pass
.github/skills/0-external-sig-report/run.sh <myapp>.json \
  --consolidation \
  --output-format json

# Generate rustdoc into the repo-root reports directory
.github/skills/0-external-sig-report/run.sh \
  --snapshot generated \
  --snapshot-output reports/rustdoc.json \
  --function-signatures

# Text output with custom minimum signature group size
.github/skills/0-external-sig-report/run.sh <myapp>.json \
  --function-signatures \
  --min-sig 5
```

## Key Files

- `.github/skills/0-external-sig-report/run.sh` -- Thin wrapper script that invokes the compiled Rust binary
- `.github/skills/0-external-sig-report/sig-report` -- Compiled Rust binary (built from source in this directory)
