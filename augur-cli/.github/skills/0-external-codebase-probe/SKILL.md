---
name: 0-external-codebase-probe
description: >
  Assembles a deterministic `ResearchSnapshot` JSON artifact from workspace
  metadata, module surfaces, test inventory, standards data, TODO state,
  module-graph data, and recent-commit provenance.
---

# run.sh

## Purpose

Assemble a deterministic `ResearchSnapshot` JSON artifact from workspace
metadata, module surfaces, test inventory, standards data, TODO state,
module-graph data, and recent-commit provenance.

## Development Build

Only needed when modifying the tool source in this directory.

```bash
cd .github/skills/0-external-codebase-probe
cargo build --release
```

## Run

```bash
.github/skills/0-external-codebase-probe/run.sh --src <repo-relative-rust-path> [--graph <path>] [--commit <path>] [--standards <path>] [--todos <path>]
```

## Usage

- `--src <repo-relative-rust-path>` - Repository-relative Rust path to scan; required
- `--graph <path>` - Path to module-graph JSON output (optional)
- `--commit <path>` - Path to recent-commit JSON from `global-git-operator` (optional; omit to mark the snapshot degraded)
- `--standards <path>` - Path to standards-feed JSON (optional; omit to mark the snapshot degraded)
- `--todos <path>` - Path to todo-state JSON (optional; omit to mark the snapshot degraded)
- `--request <path>` - Path to an `AssemblyRequest` JSON file; overrides `--src`, `--graph`, `--commit`, `--standards`, and `--todos`

## Examples

```bash
# Assemble snapshot with all feeds
.github/skills/0-external-codebase-probe/run.sh \
  --src <repo-relative-rust-path> \
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

