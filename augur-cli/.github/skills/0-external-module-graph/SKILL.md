---
name: 0-external-module-graph
description: >
  Module-level dependency graph analyzer that parses `use crate::X` imports from
  Rust source to build a directed module dependency graph, detect cycles, and
  report layer-ordering violations against a policy file.
---

# run.sh

## Purpose

Analyze Rust module dependencies by parsing imports, building a directed graph,
detecting cycles, and checking layer-ordering violations against a policy file.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-module-graph
cargo build --release
```

## Run

```bash
.github/skills/0-external-module-graph/run.sh [<repo-relative-rust-path>] [--format <format>] [--output <file>] [--layers] [--no-violations] [--config <yaml>] [--baseline-json <file>]
```

## Usage

- `[<repo-relative-rust-path>]` - Repository-relative Rust path to analyze (default: repository Rust source root)
- `--format <format>` - Output format: `text` | `dot` | `json` (default: `text`)
- `--output <file>` - Write output to file instead of stdout (optional)
- `--layers` - Include layer assignment table in text output (optional)
- `--no-violations` - Skip violation checks; emit graph structure only (optional)
- `--config <yaml>` - Path to YAML layer-policy override file (default: `config/layers.yaml`)
- `--baseline-json <file>` - Path to baseline JSON from previous run for edge-diff output (optional)

Prefer `--format json` for model-facing or summary-driven runs. Use `text`
or `dot` only when those specific representations are needed.

## Examples

```bash
# Generate graph in text format with violations check
.github/skills/0-external-module-graph/run.sh <repo-relative-rust-path> --format text

# Generate graph as Graphviz DOT for visualization
.github/skills/0-external-module-graph/run.sh <repo-relative-rust-path> --format dot --output graph.dot

# Generate JSON output with layer assignments
.github/skills/0-external-module-graph/run.sh <repo-relative-rust-path> --format json --layers

# Generate with custom policy and compare to baseline
.github/skills/0-external-module-graph/run.sh <repo-relative-rust-path> --config custom-layers.yaml --baseline-json previous-graph.json
```

## Key Files

- `run.sh` - Canonical wrapper for graph analysis runs
- `config/layers.yaml` - Default layer policy configuration
