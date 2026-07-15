---
name: 0-external-topology-extractor
description: >
  Deterministic analyzer that reads Rust wiring code and produces
  .github/local/system-actor-graph.yml documenting the complete actor topology.
  Use to regenerate or verify topology after wiring changes.
---

# topology-extractor

## When to use

Use this skill when you need the complete actor topology extracted from Rust wiring
code for planning or review purposes. This is the canonical way to generate or
update `.github/local/system-actor-graph.yml`.

## Purpose

Discovers all actor spawn/build calls in wiring source files, assigns architectural
layers based on wiring file conventions, detects handle-typed dependencies between
actors, and produces a YAML file matching the `0-system-topology` schema. Reports
ambiguities that require human review (generic parameters, unresolved types). The
tool is read-only on source code: no `src/` or `tests/` files are modified. Only
writes to the path specified by `--output`. Findings and actors are sorted for
stable output.

## Run

```bash
.github/skills/0-external-topology-extractor/run.sh <WIRING_PATH> [options]
```

## Arguments

| Argument | Description | Default |
|---|---|---|
| `<WIRING_PATH>` | Path to the wiring directory (e.g., `crates/augur-app/src/wiring`) | (required) |
| `-o, --output <OUTPUT>` | Output path for the generated YAML topology file | `.github/local/system-actor-graph.yml` |
| `-f, --format <FORMAT>` | Output format for the extraction report (`text`, `json`) | `text` |
| `--dry-run` | Do not write the output file; only print the report | (flag, off by default) |
| `--crate-root <CRATE_ROOT>` | Target crate root for module resolution | workspace root |
| `-h, --help` | Print help information | — |
| `-V, --version` | Print version information | — |

## Output

The tool produces two outputs:

1. **Stdout report** - A formatted extraction report in `text` or `json` format
   (controlled by `--format`) describing discovered actors, layers, and
   dependencies.

2. **YAML topology file** - Written to the path specified by `--output` (default:
   `.github/local/system-actor-graph.yml`). The YAML follows the `0-system-topology`
   schema.

**Exit codes:**
- `0` = no error findings
- `1` = error findings present
- `2` = runtime/config error

Read-only on source code: the tool never modifies `src/` or `tests/` files.
Only writes to the path specified by `--output`. The `--dry-run` flag suppresses
file output entirely. Findings and actors are sorted for stable output.

## Examples

Extract topology from a wiring directory and write the default output file:

```bash
.github/skills/0-external-topology-extractor/run.sh crates/augur-app/src/wiring
```

Dry-run with JSON report output to inspect results without writing:

```bash
.github/skills/0-external-topology-extractor/run.sh crates/augur-app/src/wiring --dry-run -f json
```

Specify a custom output path and crate root:

```bash
.github/skills/0-external-topology-extractor/run.sh crates/other-app/src/wiring -o /tmp/topology.yml --crate-root crates/other-app
```

## Key Files

- `.github/skills/0-external-topology-extractor/run.sh` - Canonical wrapper for the topology extractor tool
- `.github/local/system-actor-graph.yml` - Default output path for the generated topology