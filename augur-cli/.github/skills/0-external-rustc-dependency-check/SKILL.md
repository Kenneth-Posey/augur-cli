---
name: 0-external-rustc-dependency-check
description: >
  Cargo metadata and rustc-resolved dependency-direction checker that validates
  package-layer flow and forbidden edges from a YAML policy.
---

# rustc-dependency-check

## When to use

Use this skill when you need dependency-direction validation based on Cargo-resolved edges instead of source-text import parsing.

## Purpose

Validates package-layer dependency direction and forbidden edges from a YAML layer policy against Cargo metadata resolved by `cargo metadata`. Reports violations of the declared policy — packages depending on packages from higher or forbidden layers — in text or JSON format.

## Run

```bash
.github/skills/0-external-rustc-dependency-check/run.sh [WORKSPACE_ROOT] [OPTIONS]
```

## Arguments

| Argument | Description | Default |
|---|---|---|
| `WORKSPACE_ROOT` | Workspace root directory that contains `Cargo.toml` | `.` |

| Option | Description | Default |
|---|---|---|
| `--manifest-path <MANIFEST_PATH>` | Explicit path to `Cargo.toml`. Overrides `WORKSPACE_ROOT/Cargo.toml` when set | None |
| `--format <FORMAT>` | Output format for findings | `text` (choices: `text`, `json`) |
| `--output <OUTPUT>` | Optional file path to write output to instead of stdout | None |
| `--config <CONFIG>` | Optional path to YAML layer policy. Defaults to checked-in config at `.github/skills/0-external-rustc-dependency-check/config/layers.yaml` | Checked-in config |
| `--fail-on-violations <FAIL_ON_VIOLATIONS>` | Whether dependency-direction findings should fail the command (exit non-zero) | `yes` (choices: `yes`, `no`) |
| `-h`, `--help` | Print help | |
| `-V`, `--version` | Print version | |

## Output

The tool produces a report of all packages that violate the declared layer policy, with each violation listing the source package, its layer, the target package, its layer, and the policy rule that was violated.

**Exit codes:**
- `0` — No violations found (or `--fail-on-violations no`)
- Non-zero — Violations found and `--fail-on-violations yes`

**Output formats:**
- `text` (default) — Human-readable formatted report
- `json` — Machine-readable JSON for downstream processing

## Examples

```bash
# Analyze the current workspace with text output
.github/skills/0-external-rustc-dependency-check/run.sh .

# Analyze a specific workspace with JSON output
.github/skills/0-external-rustc-dependency-check/run.sh path/to/workspace --format json

# Analyze a specific manifest with custom policy and fail-off
.github/skills/0-external-rustc-dependency-check/run.sh . \
  --manifest-path path/to/Cargo.toml \
  --config path/to/layers.yaml \
  --fail-on-violations no

# Write output to a file
.github/skills/0-external-rustc-dependency-check/run.sh . --output findings.txt
```

## Key Files

- `.github/skills/0-external-rustc-dependency-check/run.sh` — Canonical wrapper script
- `.github/skills/0-external-rustc-dependency-check/config/layers.yaml` — Default package-layer policy

