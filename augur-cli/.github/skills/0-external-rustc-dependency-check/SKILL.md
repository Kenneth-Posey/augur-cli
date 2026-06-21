---
name: 0-external-rustc-dependency-check
description: >
  Cargo metadata and rustc-resolved dependency-direction checker that validates
  package-layer flow and forbidden edges from a YAML policy.
---

# 0-external-rustc-dependency-check

## When to use

Use this skill when you need dependency-direction validation based on
Cargo-resolved edges instead of source-text import parsing.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-rustc-dependency-check
cargo build --release
```

## Run

```bash
.github/skills/0-external-rustc-dependency-check/run.sh [<workspace-root>] [--manifest-path <cargo-toml>] [--format <format>] [--config <yaml>] [--output <file>] [--fail-on-violations <yes|no>]
```

## Usage

- `[<workspace-root>]` - Directory containing the target `Cargo.toml` (default: `.`)
- `--manifest-path <cargo-toml>` - Explicit manifest path override (optional)
- `--format <format>` - Output format: `text` | `json` (default: `text`)
- `--config <yaml>` - Path to YAML package-layer policy (default: checked-in `config/layers.yaml`)
- `--output <file>` - Write output to file instead of stdout (optional)
- `--fail-on-violations <yes|no>` - Exit non-zero on findings (default: `yes`)

## Examples

```bash
# Analyze the current workspace with text output
.github/skills/0-external-rustc-dependency-check/run.sh .

# Analyze a specific workspace with JSON output
.github/skills/0-external-rustc-dependency-check/run.sh path/to/workspace --format json

# Analyze a specific manifest with custom policy
.github/skills/0-external-rustc-dependency-check/run.sh . \
  --manifest-path path/to/Cargo.toml \
  --config path/to/layers.yaml \
  --format json
```

## Key Files

- `run.sh` - Canonical wrapper
- `config/layers.yaml` - Default package-layer policy

