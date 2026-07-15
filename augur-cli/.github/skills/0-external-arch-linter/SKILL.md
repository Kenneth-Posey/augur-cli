---
name: 0-external-arch-linter
description: >
  Deterministic architecture-structure linter for Rust projects that validates
  module layout, detects dependency-direction violations, identifies circular
  imports, flags path leaks and repository-relative source-root references, and
  ensures acyclic module graphs.
---

# arch-linter

## When to use

Use this skill when you need deterministic verification of Rust project module
layout, dependency direction, and cyclic-import rules in a CI or review pipeline.

## Purpose

Lint Rust projects for module layout, dependency direction, circular imports,
path leaks, repository-relative source-root reference leaks, and acyclic module
graphs.

## Run

```bash
.github/skills/0-external-arch-linter/run.sh [SRC_ROOT] [--output-format <format>] [--fail-on-findings <yes|no>]
```

## Arguments

| Argument | Description | Default |
|---|---|---|
| `SRC_ROOT` | Path to the Rust `src/` directory to analyze | `src` |
| `--output-format <format>` | Output format: `text` or `json` | `text` |
| `--fail-on-findings <yes\|no>` | Return non-zero exit code when findings are present: `yes` or `no` | `yes` |
| `-h`, `--help` | Print help | - |
| `-V`, `--version` | Print version | - |

## Output

Produces a deterministic lint report in either human-readable text or
machine-readable JSON format. Exit codes:

- `0` -- No findings, or `--fail-on-findings no` was set.
- `1` -- Findings were present and `--fail-on-findings yes` (the default).

### Text format example

```
Architecture Lint Report: <SRC_ROOT>

Findings (3 total):
  1. Circular dependency: actor -> wiring -> actor
  2. Wrong-direction import: handlers -> domain (should be: domain -> handlers)
  3. Layer violation: ui imports core (skipping services layer)

Status: FAIL
Exit code: 1
```

### JSON format

When `--output-format json` is used, the report is emitted as deterministic
JSON suitable for downstream processing.

## Examples

```bash
# Lint the default src/ directory
.github/skills/0-external-arch-linter/run.sh

# Lint a specific source directory
.github/skills/0-external-arch-linter/run.sh src/my_crate

# JSON output for downstream processing
.github/skills/0-external-arch-linter/run.sh src --output-format json

# Generate report but exit 0 even with findings
.github/skills/0-external-arch-linter/run.sh src --fail-on-findings no

# Fail on findings (exit code 1 if violations detected)
.github/skills/0-external-arch-linter/run.sh src --fail-on-findings yes
```

## Key Files

- `.github/skills/0-external-arch-linter/run.sh` -- Canonical wrapper for arch-linter

