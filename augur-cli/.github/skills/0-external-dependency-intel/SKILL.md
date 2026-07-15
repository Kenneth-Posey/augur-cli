---
name: 0-external-dependency-intel
description: >
  Deterministic dependency-intelligence analyzer that consumes `cargo metadata`
  and optional `cargo audit --json` output to emit structured package, advisory,
  and duplicate-version findings.
---

# dependency-intel

## When to use

Use this skill to analyze Rust dependencies from `cargo metadata` and optional
`cargo audit --json` output. It reports package inventory, advisories,
dependency trees, and duplicate versions.

## Purpose

Consumes `cargo metadata --format-version 1` and optional `cargo audit --json`
output to emit structured package and advisory intelligence. Supports four
output modes: metadata, advisory, tree, and duplicate-versions.

## Run

```bash
.github/skills/0-external-dependency-intel/run.sh <METADATA> [OPTIONS]
```

## Arguments

| Argument / Option | Description |
|---|---|
| `<METADATA>` | Path to `cargo metadata --format-version 1` JSON output (required) |
| `--audit <AUDIT>` | Path to `cargo audit --json` output (optional) |
| `--mode <MODE>` | Output mode. Possible values: `metadata` (full IntelReport as JSON), `advisory` (advisory findings only), `tree` (workspace tree text view), `duplicate-versions` (packages with duplicate resolved versions). Default: `metadata` |
| `--output <OUTPUT>` | Write output to this file instead of stdout (optional) |
| `-h`, `--help` | Print help |
| `-V`, `--version` | Print version |

## Output

Default output is written to stdout. When `--output <file>` is provided, output
is written to the specified file path.

- **metadata mode**: JSON report containing package inventory, resolved versions,
  and dependency relationships.
- **advisory mode**: JSON report containing advisory findings from `cargo audit`.
- **tree mode**: Plain-text workspace dependency tree view.
- **duplicate-versions mode**: JSON report listing packages with duplicate
  resolved versions.

Exit code `0` on success, non-zero on error.

## Examples

Generate cargo metadata and run analysis:

```bash
cargo metadata --format-version 1 > metadata.json
cargo audit --json > audit.json
```

Run dependency analysis (output to stdout):

```bash
.github/skills/0-external-dependency-intel/run.sh metadata.json --audit audit.json --mode metadata
```

Extract advisory findings:

```bash
.github/skills/0-external-dependency-intel/run.sh metadata.json --audit audit.json --mode advisory
```

View dependency tree:

```bash
.github/skills/0-external-dependency-intel/run.sh metadata.json --mode tree
```

Detect duplicate versions:

```bash
.github/skills/0-external-dependency-intel/run.sh metadata.json --mode duplicate-versions
```

Write advisory findings to a custom file:

```bash
.github/skills/0-external-dependency-intel/run.sh metadata.json --audit audit.json --mode advisory --output reports/custom-advisories.json
```

## Key Files

- `.github/skills/0-external-dependency-intel/run.sh` - Canonical wrapper for dependency-intel
