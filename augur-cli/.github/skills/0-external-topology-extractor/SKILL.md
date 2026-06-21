---
name: 0-external-topology-extractor
description: >
  Deterministic analyzer that reads Rust wiring code and produces
  .github/local/system-actor-graph.yml documenting the complete actor topology.
  Use to regenerate or verify topology after wiring changes.
---

# 0-external-topology-extractor

## When to use

Use this skill when you need the complete actor topology extracted from Rust wiring
code for planning or review purposes. This is the canonical way to generate or
update `.github/local/system-actor-graph.yml`.

## Scope

- Discovers all actor spawn/build calls in wiring source files
- Assigns architectural layers based on wiring file conventions
- Detects handle-typed dependencies between actors
- Produces a YAML file matching the `0-system-topology` schema
- Reports ambiguities that require human review (generic parameters, unresolved types)

## Run

```bash
.github/skills/0-external-topology-extractor/run.sh <wiring-path> [options]
```

## Arguments

- `<wiring-path>` - Path to the wiring directory (e.g., `crates/augur-app/src/wiring`)
- `-o, --output <path>` - Output path for the YAML file (default: `.github/local/system-actor-graph.yml`)
- `-f, --format <format>` - Output format: `text` | `json` (default: `text`)
- `--dry-run` - Do not write the output file; only print the report
- `--crate-root <path>` - Target crate root for module resolution (default: workspace root)

## Determinism and safety

- Read-only on source code: no `src/` or `tests/` files are modified
- Only writes to the path specified by `--output`
- Findings and actors are sorted for stable output
- Exit codes: `0` = no error findings, `1` = error findings present, `2` = runtime/config error
- The generated YAML follows the `0-system-topology` skill schema

## Key Files

- `run.sh` - Canonical wrapper for the topology extractor tool
- The extracted YAML is written to `.github/local/system-actor-graph.yml` by default