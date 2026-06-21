---
name: 0-external-dependency-intel
description: >
  Deterministic dependency-intelligence analyzer that consumes `cargo metadata`
  and optional `cargo audit --json` output to emit structured package, advisory,
  and duplicate-version findings.
---

# 0-external-dependency-intel

## When to use

Use this skill to analyze Rust dependencies from `cargo metadata` and optional
`cargo audit --json` output. It reports package inventory, advisories,
dependency trees, and duplicate versions.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-dependency-intel
cargo build --release
```

## Run

```bash
.github/skills/0-external-dependency-intel/run.sh <metadata.json> [--audit <audit.json>] [--mode <mode>] [--output <file>]
```

## Usage

- `<metadata.json>` - Path to `cargo metadata --format-version 1` JSON output; required
- `--audit <path>` - Path to `cargo audit --json` output (optional)
- `--mode <mode>` - Output mode: `metadata` | `advisory` | `tree` | `duplicate-versions` (default: `metadata`)
- `--output <file>` - Write output to a custom file (optional). When omitted, defaults by mode under `reports/`: `dependency-intel-metadata.json`, `advisories.json`, `dependency-tree.txt`, or `dependency-duplicate-versions.json`

## Examples

```bash
# Generate cargo metadata and run analysis
cargo metadata --format-version 1 > metadata.json
cargo audit --json > audit.json

# Run dependency analysis (writes to reports/dependency-intel-metadata.json by default)
.github/skills/0-external-dependency-intel/run.sh metadata.json --audit audit.json --mode metadata

# Extract advisory findings (writes to reports/advisories.json by default)
.github/skills/0-external-dependency-intel/run.sh metadata.json --audit audit.json --mode advisory

# View dependency tree (writes to reports/dependency-tree.txt by default)
.github/skills/0-external-dependency-intel/run.sh metadata.json --mode tree

# Detect duplicate versions (writes to reports/dependency-duplicate-versions.json by default)
.github/skills/0-external-dependency-intel/run.sh metadata.json --mode duplicate-versions

# Write advisory findings to a custom path under reports/
.github/skills/0-external-dependency-intel/run.sh metadata.json --audit audit.json --mode advisory --output reports/custom-advisories.json
```

## Key Files

- `run.sh` - Canonical wrapper for dependency intel
