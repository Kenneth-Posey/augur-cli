---
name: 0-system-topology
description: >
  Schema and usage rules for .github/local/system-actor-graph.yml. Read this
  skill before using the topology file during planning or review. Covers schema
  fields, update obligations, and how to incorporate topology data into
  dependency graphs and wiring plans.
---

# Skill: 0-system-topology

## Purpose

`.github/local/system-actor-graph.yml` is the maintained actor topology for the
project's actor-based system. It records all actors, their crate and module locations, their
architectural layer, their handle types, and the directed handle-dependency edges
between them. It is not generated at query time; it is kept current by the team
as part of wiring changes.

Read this file during Stage 2 planning when a feature touches existing actors or
requires new actors. Do not read `src/` to discover topology; use this file
instead.

## Schema Summary

The file has two top-level keys: `actors` and `edges`.

**actors** entries record:
- `name` — primary key; used in edge `from`/`to` references
- `crate` — Rust package name (e.g. "my-app-core")
- `module_path` — repo-relative path to the actor module directory
- `layer` — one of `infrastructure`, `domain`, `planning`, `tui`
- `handle_type` — the concrete Handle struct type callers hold
- `spawn_fn` — the wiring call that constructs this actor

**edges** entries record directed handle dependencies:
- `from` — the dependent actor (holds the handle)
- `to` — the dependency actor (whose handle is held)
- `handle_type` — must match the `to` actor's `handle_type`
- `via_field` — field name in the spawn config, or generic parameter description
- `message_enum` — optional; the command enum for this channel

## Layer Mapping

The `layer` values map to wiring sub-modules typical of actor-based Rust
applications (the exact module structure depends on the project's wiring
conventions):

| Layer          | Typical Source                     | Characteristics                                  |
|----------------|------------------------------------|--------------------------------------------------|
| infrastructure | wiring/infrastructure.rs           | No handle dependencies on other actors in graph  |
| domain         | wiring/domain.rs (SpawnedDomainActors) | Depends on infrastructure handles             |
| planning       | wiring/domain.rs (SpawnedPlanningActors) | Stateless at startup, minimal deps           |
| tui            | wiring/tui_wiring.rs               | Depends on all lower layers                      |

## Reading the Topology During Planning

When a feature modifies or extends an existing actor, or adds a new actor that
takes handles from existing actors:

1. Read `.github/local/system-actor-graph.yml` in full.
2. Identify all existing actor nodes that the feature will interact with:
   actors it adds edges to/from, actors whose handle types it introduces,
   actors whose spawn config it modifies.
3. Include those existing actor nodes in the feature's `dependency-graph.md`
   as pre-existing nodes. Mark them with a comment such as
   `# existing — not introduced by this feature`.
4. Draw the new edges proposed by the feature on top of the existing nodes.
5. Check that no new edge creates a cycle when combined with existing edges.

## Checking for Cycles Against Existing Topology

A cycle exists when following edges from any node eventually returns to that
same node. When validating a feature's proposed new edges:

1. Build the full combined edge list: all existing edges from
   `system-actor-graph.yml` plus all proposed new edges from the feature's
   `dependency-graph.md`.
2. Walk the combined graph. Any path that returns to its starting node is a
   cycle.
3. A new edge `from: A, to: B` introduces a cycle if there is already a
   path from B to A in the existing topology.

Report any detected cycle as a hard blocker. Do not proceed with planning until
the cycle is resolved.

## Update Obligations

Update `.github/local/system-actor-graph.yml` when any of the following occur
in a Stage 3 wiring phase:

- A new actor is added to the wiring layer
- An existing actor's spawn config gains or loses a handle dependency
- An actor is removed
- An actor's handle type is renamed
- An actor's layer assignment changes (e.g. moved from infrastructure to domain)

The update must be committed in the same changeset as the wiring code change.
Do not defer topology updates.

## Verification

During Stage 4 review, `review-architecture-checker` verifies that the topology
file is consistent with the actual wiring code. If wiring source
files were modified in the changeset and `system-actor-graph.yml` was not
updated, that is a `high` severity finding.