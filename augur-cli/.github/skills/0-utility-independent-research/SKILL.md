---
name: 0-utility-independent-research
description: >
  Builds a deterministic research snapshot for planning and debugging.
  Use when a workflow needs one canonical workspace snapshot instead of
  running ad hoc queries.
---

# Independent Research Skill

## Tool Sequence

Run `codebase-probe` before planning or debugging to assemble the snapshot:

```sh
# Assemble a complete snapshot (all feeds available)
.github/skills/0-external-codebase-probe/run.sh \
    --src src \
    --standards standards.json \
    --todos todos.json \
    --graph graph.json \
    --commit commit.json \
    > research-snapshot.json

# Assemble from a pre-built request file
.github/skills/0-external-codebase-probe/run.sh \
    --request assembly_request.json \
    > research-snapshot.json
```

The runner collects feeds in this order:

1. **Workspace metadata** - from `Cargo.toml` at the source root.
2. **Module surfaces** - public symbols from every `.rs` file via `syn`.
3. **Test inventory** - mirrored test coverage discovered for the same scope.
4. **Standards feed** - JSON input passed with `--standards`.
5. **Todo state** - JSON input passed with `--todos`.
6. **Module-graph reference** - JSON input passed with `--graph` (produced by `dependency-intel` or `plan-dependency-plan-evaluator`).
7. **Recent-commit artifact** - JSON input passed with `--commit`, typically
   produced by `global-git-operator`.
8. **Assembly** - all feeds combined into one `ResearchSnapshot` JSON.

## Key Files

- `README.md` - overview and usage notes

## Snapshot Storage

If `.github/local/directories.md` defines a research snapshot path, write
assembled snapshots there. Do not store snapshots inside `src/`, `tests/`, or
`target/`.

## Degraded Mode

When `--standards`, `--todos`, or `--commit` is absent or points to an
unreadable file, the assembled snapshot has `provenance.is_degraded = true`.
Consumers must check this flag before treating the artifact as complete.

Degraded snapshots are still valid for planning work, but the missing
standards, todo, or commit feed must be acknowledged explicitly. Reduced
snapshots are never a silent substitute for the full feed set.

## Consumer Contract

Consumers of `research-snapshot.json` should:

1. Load and read `research-snapshot.json` first.
2. Use `snapshot.surfaces` for the public symbol inventory.
3. Use `snapshot.graph_ref.file_path` to locate the module-graph JSON for
   dependency-direction facts.
4. Use `snapshot.recent_commit` for commit-provenance context.
5. Open individual source files **only** when the snapshot leaves a specific
   question unresolved.
6. If the snapshot is absent or its `provenance.is_degraded` flag is `true`,
   note the gap and fall back to direct file reads for the missing feed only.

## Retention Policy

If `.github/local/rules.md` defines research snapshot retention rules, follow
them.

## Snapshot Reuse

When reusing an existing snapshot instead of running a fresh
`codebase-probe`, apply this contract:

1. Reuse an existing snapshot only when it contains the expanded feed set
   (workspace, surfaces, tests, standards, todos, graph, and commit) or when
   `provenance.is_degraded = true` makes the missing `--standards`, `--todos`,
   or `--commit` feed explicit.
2. If the snapshot is absent, or if the expanded feed set is incomplete without
   explicit degraded acknowledgement, trigger a fresh `codebase-probe` run
   instead of silently reusing the reduced artifact.
3. When a degraded snapshot is reused, treat it as partial evidence only and
   fall back to direct file reads for the acknowledged missing feed.
4. The research snapshot is a read-only optimization input. Do not use it to
   reconstruct orchestration or task state.

## Boundary with module-graph (plan 0154)

`codebase-probe` collects **public-surface facts** (symbol names and kinds)
but does **not** analyze import edges or dependency direction. That remains
with `module-graph`. The snapshot stores a reference path to the module-graph
JSON rather than re-analyzing it.

## External Tools

This skill uses the following external tools:

- [`0-external-codebase-probe`](../0-external-codebase-probe/SKILL.md) - Assemble deterministic research snapshots from workspace metadata, module surfaces, and feeds
- [`0-external-dependency-intel`](../0-external-dependency-intel/SKILL.md) - Analyze cargo metadata and audit output for advisory and duplicate-version findings
- [`0-external-module-graph`](../0-external-module-graph/SKILL.md) - Build directed module dependency graphs, detect cycles, and validate layer ordering
- [`0-external-sig-report`](../0-external-sig-report/SKILL.md) - Identify API consolidation opportunities, signature duplication, and refactoring priorities
