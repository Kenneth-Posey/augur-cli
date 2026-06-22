---
name: 0-external-cargo-diagnostics
description: >
  Deterministic pipeline that normalizes compiler, clippy, and test diagnostics
  from `cargo check`, `cargo clippy`, or nextest JUnit XML into a single
  machine-readable JSON report.
---

# run.sh

## Purpose

Deterministically normalize compiler, clippy, and test diagnostics from
`cargo check`, `cargo clippy`, or nextest JUnit XML into one JSON report.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-cargo-diagnostics
cargo build --release
```

## Run

```bash
.github/skills/0-external-cargo-diagnostics/run.sh <input-file> [--mode <mode>] [--output <file>]
```

## Usage

- `<input-file>` - Compiler/clippy JSON, JUnit XML, or test-list text. Required.
- `--mode <mode>` - Input format: `cargo-json` | `nextest-junit` | `test-list` (default: `cargo-json`)
- `--output <file>` - Write output to a custom file (default when omitted: `reports/diagnostics.json`)

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

# Parse fallback test list
.github/skills/0-external-cargo-diagnostics/run.sh test-list.txt --mode test-list --output reports/diagnostics.json
```

## Key Files

- `run.sh` - Canonical wrapper for cargo diagnostics
