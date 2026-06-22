---
name: 0-external-syn-analyzer
description: >
  AST-based Rust code quality analyzer that parses source files using `syn` and
  reports violations including oversized parameter lists, oversized struct field
  counts, deep if/else-if chains, high cyclomatic complexity, long function
  bodies, unexplained magic literals, missing docs, bare primitive signatures,
  repeated trait bounds, and deep boolean formulas.
---

# run.sh

## Purpose

Analyze Rust source with `syn` and report violations such as oversized
parameter lists and structs, deep if/else-if chains, high cyclomatic
complexity, long function bodies, unexplained magic literals, missing docs,
bare primitive signatures, repeated trait bounds, and deep boolean formulas.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-syn-analyzer
cargo build --release
```

## Run

```bash
.github/skills/0-external-syn-analyzer/run.sh [target-path] [--format <format>] [--reports <list>] [--max-params <n>] [--max-fields <n>] [--max-lines <n>] [--max-chain <n>] [--max-complexity <n>] [--magic-threshold <n>] [--rule-id <id>] [--severity <level>] [--path <fragment>]
```

## Usage

- `[target-path]` - Path to analyze (default: `src`)
- `--format <format>` - Output format: `text` | `json` (default: `text`)
- `--reports <list>` - Comma-separated report selection (default: `all`)
- `--max-params <n>` - Maximum non-self parameters allowed (default: 3)
- `--max-fields <n>` - Maximum struct fields allowed (default: 5)
- `--max-lines <n>` - Maximum function body lines allowed (default: 50)
- `--max-chain <n>` - Maximum if/else-if chain depth allowed (default: 3)
- `--max-complexity <n>` - Maximum cyclomatic complexity allowed (default: 5)
- `--magic-threshold <n>` - Numeric literals above this value are flagged (default: 9)
- `--rule-id <id>` - Filter findings by rule ID (repeatable)
- `--severity <level>` - Filter findings by severity (repeatable)
- `--path <fragment>` - Filter findings whose source path contains this fragment (repeatable)

## Examples

```bash
# Analyze src directory with default thresholds
.github/skills/0-external-syn-analyzer/run.sh src

# JSON output with custom parameter threshold
.github/skills/0-external-syn-analyzer/run.sh src --format json --max-params 5

# Filter for specific findings
.github/skills/0-external-syn-analyzer/run.sh src --rule-id params --severity warning

# Analyze specific path with lowered complexity threshold
.github/skills/0-external-syn-analyzer/run.sh src/actor/ --max-complexity 8 --path "actor.rs"

# Custom thresholds across all metrics
.github/skills/0-external-syn-analyzer/run.sh src \
  --max-params 4 \
  --max-fields 8 \
  --max-lines 100 \
  --max-chain 4 \
  --max-complexity 12 \
  --magic-threshold 15
```

## Key Files

- `run.sh` - Canonical wrapper for syn analyzer
