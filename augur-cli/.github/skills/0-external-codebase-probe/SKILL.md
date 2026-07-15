---
name: 0-external-codebase-probe
description: >
  Assembles a deterministic `ResearchSnapshot` JSON artifact from workspace
  metadata, module surfaces, test inventory, standards data, TODO state,
  module-graph data, and recent-commit provenance.
---

# codebase-probe

## When to use

Use this skill when you need a consolidated research snapshot combining Rust
source structure, dependency metadata, test inventory, standards data, TODO
state, module graph data, and recent-commit provenance into a single JSON
artifact for review or planning handoffs.

## Purpose

Assemble a deterministic `ResearchSnapshot` JSON artifact from workspace
metadata, module surfaces, test inventory, standards data, TODO state,
module-graph data, and recent-commit provenance.

## Run

```bash
.github/skills/0-external-codebase-probe/run.sh [OPTIONS]
```

When modifying the tool source, build with:

```bash
cd .github/skills/0-external-codebase-probe
cargo build --release
```

## Arguments

- `--src <SRC>` - Path to the Rust source tree to analyze. Optional; defaults to `src` when omitted.
- `--graph <GRAPH>` - Path to a module-graph JSON output file. Optional. When omitted, the snapshot is still produced without a graph reference. Graph absence alone does not degrade the snapshot. The graph reference is included in the snapshot feeds without performing graph analysis here.
- `--commit <COMMIT>` - Path to a recent-commit JSON file produced by `git-operator`. Optional. When absent, the snapshot is marked as degraded because commit provenance is required for a complete artifact.
- `--standards <STANDARDS>` - Path to a standards-feed JSON file. Optional. When absent, the snapshot is marked degraded and emits an empty standards feed so the missing input remains explicit in the output artifact.
- `--todos <TODOS>` - Path to a todo-state JSON file. Optional. When absent, the snapshot is marked degraded and emits an empty todo-state feed so the missing input remains explicit in the output artifact.
- `--request <REQUEST>` - Path to an `AssemblyRequest` JSON file. When provided, `--src`, `--graph`, `--commit`, `--standards`, and `--todos` are ignored and all feed inputs come from the JSON file directly.
- `-h, --help` - Print help (see a summary with `-h`).
- `-V, --version` - Print version.

## Output

The tool writes a single `ResearchSnapshot` JSON artifact to stdout. The
artifact is deterministic for a given set of inputs. Exit code 0 indicates
success; any non-zero exit code indicates a failure to assemble the snapshot.
When optional feeds (`--commit`, `--standards`, `--todos`) are omitted, the
snapshot is marked as degraded and the missing inputs remain explicit in the
output artifact through empty feed entries.

## Examples

```bash
# Assemble snapshot with all feeds
.github/skills/0-external-codebase-probe/run.sh \
  --src src \
  --standards standards.json \
  --todos todos.json \
  --graph module-graph.json \
  --commit recent-commit.json > research-snapshot.json

# Assemble snapshot from request file
.github/skills/0-external-codebase-probe/run.sh \
  --request assembly_request.json > research-snapshot.json
```

## Key Files

- `run.sh` - Canonical wrapper for codebase probe

