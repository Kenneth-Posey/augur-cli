---
name: 0-external-consolidator
description: >
  Call-graph analysis tool that detects dead code, duplicate functions, and
  chain-collapse opportunities in a Rust source tree.
---

# consolidator

## When to use

Use this skill when you need to identify dead code, duplicate functions, or
collapsible call chains in a Rust project during architecture review or
technical-debt assessment.

## Purpose

Analyze a Rust source tree's call graph to detect consolidation opportunities:
- **Dead code**: functions with no callers (confidence-scored)
- **Duplicate functions**: functions with identical normalized signatures in the same layer
- **Chain-collapse**: linear call chains that could be collapsed without behavioral change

## Run

```bash
.github/skills/0-external-consolidator/run.sh [SOURCE_PATH] [OPTIONS]
```

To build the tool from source after modifying it:

```bash
cd .github/skills/0-external-consolidator
cargo build --release
```

## Arguments

| Argument | Description | Default |
|---|---|---|
| `SOURCE_PATH` | Path to the directory containing the `Cargo.toml` to analyze | `.` |
| `--output-format <OUTPUT_FORMAT>` | Output format for the report. Possible values: `text`, `json` | `text` |
| `--min-confidence <MIN_CONFIDENCE>` | Minimum confidence threshold (0.0–1.0) for reported opportunities | `0` |
| `--no-color` | Disable color output (currently unused; reserved for future formatting) | — |
| `-h`, `--help` | Print help | — |
| `-V`, `--version` | Print version | — |

## Output

The tool analyzes the Rust source tree and produces a report identifying
consolidation opportunities across three categories:

- **Dead code** — functions with no callers, each with a confidence score
- **Duplicates** — functions with identical normalized signatures in the same layer
- **Chain-collapses** — linear call chains that could be collapsed without behavioral change

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Analysis completed successfully |

### Text format (default)

Human-readable report with sections for dead code, duplicates, and
chain-collapse candidates. Each finding includes function ID, module path,
confidence score, and explanation.

### JSON format

Machine-readable JSON suitable for downstream processing:

```json
{
  "dead_code": [...],
  "duplicates": [...],
  "chain_collapses": [...]
}
```

## Examples

```bash
# Analyze current project
.github/skills/0-external-consolidator/run.sh .

# Analyze specific directory with json output
.github/skills/0-external-consolidator/run.sh /path/to/project --output-format json

# Only show high-confidence findings
.github/skills/0-external-consolidator/run.sh . --min-confidence 0.8

# JSON output with confidence filter
.github/skills/0-external-consolidator/run.sh . --output-format json --min-confidence 0.7
```

## Key Files

- `.github/skills/0-external-consolidator/run.sh` — Canonical wrapper
