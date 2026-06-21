---
name: 0-external-consolidator
description: >
  Call-graph analysis tool that detects dead code, duplicate functions, and
  chain-collapse opportunities in a Rust source tree.
---

# run.sh

## Purpose

Analyze a Rust source tree's call graph to detect consolidation opportunities:
- **Dead code**: functions with no callers (confidence-scored)
- **Duplicate functions**: functions with identical normalized signatures in the same layer
- **Chain-collapse**: linear call chains that could be collapsed without behavioral change

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-consolidator
cargo build --release
```

## Run

```bash
.github/skills/0-external-consolidator/run.sh [source-path] [--output-format <format>] [--min-confidence <f64>] [--no-color]
```

## Usage

- `[source-path]` - Directory containing the `Cargo.toml` to analyze (default: `.`)
- `--output-format <format>` - Output format: `text` | `json` (default: `text`)
- `--min-confidence <f64>` - Minimum confidence score 0.0–1.0 for reported opportunities (default: `0.0`)
- `--no-color` - Disable color output (reserved for future use)

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

## Output Format

### Text (default)

Human-readable report with sections for dead code, duplicates, and chain-collapse candidates.
Each finding includes function ID, module path, confidence score, and explanation.

### JSON

Machine-readable JSON with the same findings, suitable for downstream processing:

```json
{
  "dead_code": [...],
  "duplicates": [...],
  "chain_collapses": [...]
}
```

## Key Files

- `run.sh` - Canonical wrapper for consolidator
