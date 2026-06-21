---
name: 0-external-arch-linter
description: >
  Deterministic architecture-structure linter for Rust projects that validates
  module layout, detects dependency-direction violations, identifies circular
  imports, flags path leaks and repository-relative source-root references, and
  ensures acyclic module graphs.
---

# run.sh

## Purpose

Lint Rust projects for module layout, dependency direction, circular imports,
path leaks, repository-relative source-root reference leaks, and acyclic module
graphs.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-arch-linter
cargo build --release
```

## Run

```bash
.github/skills/0-external-arch-linter/run.sh [repo-relative-root] [--output-format <format>] [--fail-on-findings <yes|no>]
```

## Usage

- `[repo-relative-root]` - Repository-relative root to analyze (default: repository root)
- `--output-format <format>` - Output format: `text` | `json` (default: `text`)
- `--fail-on-findings <yes|no>` - Return a non-zero exit code when findings are present: `yes` | `no` (default: `yes`)

## Examples

```bash
# Lint default repository root
.github/skills/0-external-arch-linter/run.sh

# Lint custom repository root
.github/skills/0-external-arch-linter/run.sh <repo-relative-root>

# JSON output for downstream processing
.github/skills/0-external-arch-linter/run.sh <repo-relative-root> --output-format json

# Generate report but exit 0 even with findings
.github/skills/0-external-arch-linter/run.sh <repo-relative-root> --fail-on-findings no

# Linting mode: fail on findings (exit code 1 if violations detected)
.github/skills/0-external-arch-linter/run.sh <repo-relative-root> --fail-on-findings yes
```

## Example Output (text format)

```
Architecture Lint Report: <repo-relative-root>

Findings (3 total):
  1. Circular dependency: actor → wiring → actor
  2. Wrong-direction import: handlers → domain (should be: domain → handlers)
  3. Layer violation: ui imports core (skipping services layer)

Status: FAIL
Exit code: 1
```

## Key Files

- `run.sh` - Canonical wrapper for arch linter

