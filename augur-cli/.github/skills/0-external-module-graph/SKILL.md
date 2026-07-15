---
name: 0-external-module-graph
description: >
  Module-level dependency graph analyzer that parses `use crate::X` imports from
  Rust source to build a directed module dependency graph, detect cycles, and
  report layer-ordering violations against a policy file.
---

# module-graph

## When to use

Use this skill when you need to analyze module-level import dependencies,
detect cycles, verify layer-ordering policy compliance, or produce a directed
dependency graph for architecture review.

## Purpose

Analyze Rust module dependencies by parsing `use crate::X` imports, building a
directed module dependency graph, detecting cycles, and checking layer-ordering
violations against a policy file. The tool supports layered policy enforcement
with optional baseline comparison for tracking graph changes over time.

## Run

```bash
.github/skills/0-external-module-graph/run.sh [<src>] [--format <format>] [--output <file>] [--layers] [--no-violations] [--config <yaml>] [--baseline-json <file>]
```

## Arguments

- `<src>` - Path to the Rust `src/` directory to analyze (default: `src`)
- `--format <format>` - Output format (default: `text`). Supported values: `text`, `dot`, `json`
- `--output <file>` - Write output to this file instead of stdout
- `--layers` - Include the layer assignment table in text output
- `--no-violations` - Skip violation checks; emit graph structure only
- `--config <yaml>` - Path to YAML layer-policy override file (default: `config/layers.yaml`)
- `--baseline-json <file>` - Path to baseline JSON from previous run. When supplied, the JSON output includes an added/removed edge diff section

## Output

The tool emits its result in one of three formats depending on `--format`:
- **text** - Human-readable directed graph representation with optional layer
  assignment table (when `--layers` is given) and violation report (when
  `--no-violations` is not set).
- **dot** - Graphviz DOT format suitable for rendering with `dot` or other
  graph visualization tools.
- **json** - Structured JSON with nodes, edges, cycles, violations, and an
  optional edge diff section (when `--baseline-json` is supplied).

Exit code `0` indicates success (no errors during analysis). A non-zero exit
code indicates a tool-level error.

## Examples

```bash
# Analyze default src/ directory with text output and violation checks
.github/skills/0-external-module-graph/run.sh

# Analyze specific source tree in text format
.github/skills/0-external-module-graph/run.sh src --format text

# Generate Graphviz DOT output for visualization
.github/skills/0-external-module-graph/run.sh src --format dot --output graph.dot

# Generate JSON output with layer assignments
.github/skills/0-external-module-graph/run.sh src --format json --layers

# Skip violation checks; emit graph structure only
.github/skills/0-external-module-graph/run.sh src --no-violations

# Use custom layer policy and compare against a baseline
.github/skills/0-external-module-graph/run.sh src --config custom-layers.yaml --baseline-json previous-graph.json
```

## Key Files

- `.github/skills/0-external-module-graph/SKILL.md` - This documentation file
- `.github/skills/0-external-module-graph/run.sh` - Canonical wrapper for graph analysis runs
- `.github/skills/0-external-module-graph/config/layers.yaml` - Default layer policy configuration
- `.github/skills/0-external-module-graph/README.md` - Brief entry-point README
