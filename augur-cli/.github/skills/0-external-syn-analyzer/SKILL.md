---
name: 0-external-syn-analyzer
description: >
  AST-based Rust code quality analyzer that parses source files using `syn` and
  reports violations including oversized parameter lists, oversized struct field
  counts, deep if/else-if chains, high cyclomatic complexity, long function
  bodies, unexplained magic literals, missing docs, bare primitive signatures,
  repeated trait bounds, and deep boolean formulas.
---

# syn-analyzer

## When to use

Use this skill during code quality review or CI pipelines to automatically
detect Rust-specific quality issues: oversized parameter lists, excessive
struct fields, deep if/else-if chains, high cyclomatic complexity, long
functions, magic literals, missing documentation, bare primitive signatures,
repeated trait bounds, and overly complex boolean formulas.

## Purpose

Analyze Rust source with `syn` and report violations such as oversized
parameter lists and structs, deep if/else-if chains, high cyclomatic
complexity, long function bodies, unexplained magic literals, missing docs,
bare primitive signatures, repeated trait bounds, and deep boolean formulas.

## Run

```bash
.github/skills/0-external-syn-analyzer/run.sh [TARGET_PATH] [OPTIONS]
```

## Arguments

| Argument / Flag | Description | Default |
|---|---|---|
| `TARGET_PATH` | Path to analyze | `src` |
| `-f`, `--format <FORMAT>` | Output format: `text` or `json` | `text` |
| `-r`, `--reports <REPORTS>` | Backward-compatible section selection alias. Values: `all`, `params`, `fields`, `chains`, `complexity`, `long`, `magic`, `test-doc`, `public-doc`, `primitive-signature`, `trait-bound`, `boolean-formula` | `all` |
| `--rule-id <RULE_FILTERS>` | Repeat to keep only findings with the requested stable rule ID | — |
| `--severity <SEVERITY_FILTERS>` | Repeat to keep only findings at the requested severity | — |
| `--path <PATH_FILTERS>` | Repeat to keep only findings whose source path contains this fragment | — |
| `--max-params <MAX_PARAMS>` | Maximum non-self parameters allowed | `3` |
| `--max-lines <MAX_LINES>` | Maximum function body lines allowed | `50` |
| `--max-chain <MAX_CHAIN>` | Maximum if/else-if chain depth allowed | `5` |
| `--max-complexity <MAX_COMPLEXITY>` | Maximum cyclomatic complexity allowed | `5` |
| `--max-fields <MAX_FIELDS>` | Maximum struct fields allowed | `5` |
| `--magic-threshold <MAGIC_THRESHOLD>` | Numeric literals above this value are flagged | `9` |
| `-h`, `--help` | Print help information | — |
| `-V`, `--version` | Print version information | — |

## Output

The tool reports code quality violations grouped by rule category. Each finding
includes the source file path, line number, rule identifier, severity level, and
a descriptive message.

- **Exit code 0**: All findings are within thresholds (or no findings).
- **Exit code 1**: One or more violations were detected.
- **Output format**: Plain text by default (human-readable table with file paths,
  line numbers, and violation descriptions). Use `--format json` for structured
  machine-readable output suitable for CI pipeline ingestion.

## Examples

```bash
# Analyze src directory with default thresholds
.github/skills/0-external-syn-analyzer/run.sh src

# JSON output with custom parameter threshold
.github/skills/0-external-syn-analyzer/run.sh src --format json --max-params 5

# Filter for specific findings by rule ID and severity
.github/skills/0-external-syn-analyzer/run.sh src --rule-id params --severity warning

# Analyze specific path with lowered complexity threshold and path filter
.github/skills/0-external-syn-analyzer/run.sh src/actor/ --max-complexity 8 --path "actor.rs"

# Custom thresholds across all metrics
.github/skills/0-external-syn-analyzer/run.sh src \
  --max-params 4 \
  --max-fields 8 \
  --max-lines 100 \
  --max-chain 4 \
  --max-complexity 12 \
  --magic-threshold 15

# Filter by multiple path fragments
.github/skills/0-external-syn-analyzer/run.sh src --path "domain" --path "actor"

# JSON output for CI pipeline ingestion
.github/skills/0-external-syn-analyzer/run.sh src --format json --reports params,fields,complexity
```

## Key Files

- `.github/skills/0-external-syn-analyzer/run.sh` - Canonical wrapper for syn analyzer