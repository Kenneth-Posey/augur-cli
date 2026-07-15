---
name: 0-external-cargo-diagnostics
description: >
  Deterministic pipeline that normalizes compiler, clippy, and test diagnostics
  from `cargo check`, `cargo clippy`, or nextest JUnit XML into a single
  machine-readable JSON report.
---

# cargo-diagnostics

## When to use

Use this skill when you need to convert raw compiler, clippy, or test-runner
output into a structured, deterministic JSON report for downstream analysis or
review pipelines.

## Purpose

Deterministically normalize compiler, clippy, and test diagnostics from
`cargo check`, `cargo clippy`, or nextest JUnit XML into one JSON report.

## Run

```bash
.github/skills/0-external-cargo-diagnostics/run.sh <INPUT> [--mode <MODE>] [--output <OUTPUT>]
```

## Arguments

- `<INPUT>` - Path to the input file (compiler/clippy JSON, JUnit XML, or test-list text). Required.
- `--mode <MODE>` - Input format. Possible values:
  - `cargo-json` (default): `cargo check --message-format=json` or `cargo clippy --message-format=json`
  - `nextest-junit`: Nextest JUnit XML (`--profile ci`)
  - `test-list`: Stable test-list text fallback
- `--output <OUTPUT>` - Write output to this file instead of stdout.
- `-h, --help` - Print help (see a summary with `-h`).
- `-V, --version` - Print version.

## Output

Produces a machine-readable JSON report to stdout (or to the file specified by
`--output`). Exit code 0 on success, non-zero on error.

## Examples

```bash
# Normalize cargo check output
cargo check --message-format=json > check.json
.github/skills/0-external-cargo-diagnostics/run.sh check.json --mode cargo-json

# Normalize clippy output
cargo clippy --message-format=json > clippy.json
.github/skills/0-external-cargo-diagnostics/run.sh clippy.json

# Parse nextest JUnit XML
.github/skills/0-external-cargo-diagnostics/run.sh test-results.xml --mode nextest-junit

# Parse fallback test list with custom output path
.github/skills/0-external-cargo-diagnostics/run.sh test-list.txt --mode test-list --output reports/diagnostics.json
```

## Key Files

- `.github/skills/0-external-cargo-diagnostics/run.sh` - Canonical wrapper for cargo diagnostics
