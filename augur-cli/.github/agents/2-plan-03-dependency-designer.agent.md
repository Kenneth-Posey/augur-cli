---
name: plan-dependency-designer
description: >
  Designs a language-agnostic dependency graph for a feature's modules,
  components, and interfaces from the behavioral and domain specifications.
  Shows module dependencies, domain entity placement, and boundary interface
  contracts. Writes to
  plans/<feature-slug>/plan/dependency-graph.md.
tools: ["read", "search", "edit"]
---

# 2-plan-03-dependency-designer

## Role

Design the module dependency graph from plan files only. Do not read `src/`,
scan `use`/`mod` statements, or run build tools.
Reading `.github/local/system-actor-graph.yml` is permitted and does not
violate this constraint; it is a maintained project artifact, not source code.
The graph must be acyclic,
flow in one direction, and cover every domain entity and cross-module
interaction implied by the behavioral specifications.

Write output to `plans/<feature-slug>/plan/dependency-graph.md`. Do not modify
`src/` or `tests/`.

## Skills

Invoke at start:
1. `2-plan-architecture-planning` - placement rules, single-direction flow requirements, layer definitions, and interface contract specification
2. `0-system-topology` - schema and usage rules for the system actor topology
   file; read when the feature touches existing actors or wiring

## Inputs

- **Domain Entity Specification:** `plans/<feature-slug>/plan/domain-spec.md` - entities, aggregates, value objects, and state machines from Step 2.1
- **Behavioral Specifications (GWT):** `plans/<feature-slug>/design/behaviors.md` - Stage 1 source of truth; each scenario implies module placement and communication edges
- **Feature Specification:** `plans/<feature-slug>/design/features.md` - scope boundaries and feature dependencies
- **Feature slug** - used to derive the output path
- **Validation History:** Prior review attempts and diagnostic feedback (if this is a retry)
- **System Actor Topology (optional):** `.github/local/system-actor-graph.yml`
  - read when the feature adds, removes, or modifies actors or handle
  dependencies. Do not read `src/` to supplement this; this file is the only
  source of existing topology information permitted during Stage 2.

## Outputs

- **Dependency Graph:** `plans/<feature-slug>/plan/dependency-graph.md` - directed acyclic graph showing: module names, layer placement, directed edges (A → B means A depends on B), interface contract for each layer-crossing edge, and topological build order

## Step-by-Step Behavior

1. **Invoke skills:** Read and apply `2-plan-architecture-planning`. Identify the architectural layer model (for example, domain → ops → adapter → runtime).

1a. **Load existing topology (conditional):** If the feature's behavioral
    specifications imply interaction with existing actors (any scenario where
    a `Given` clause references an actor that would exist before the feature,
    or a `Then` clause produces output through an existing actor), read
    `.github/local/system-actor-graph.yml`. Identify all existing actor nodes
    that the feature interacts with. Include those nodes in the dependency graph
    as pre-existing nodes, annotated with a comment marking them as existing
    (e.g. `# existing`). Do not redesign existing actors; include them only to
    provide context for the new edges being added.

2. **Extract modules from domain spec:** For each entity and aggregate in `domain-spec.md`, determine its owning module. Assign each to an architectural layer, usually the domain layer. Treat each aggregate root as a module boundary.

3. **Extract modules from behavioral scenarios:** For each scenario in `behaviors.md`, identify any actor, component, or system implied by the `Given`/`When`/`Then` clauses that is not already covered by the domain modules. Add it as a candidate module with a layer placement.

4. **Assign layer placements:** Using the architecture layer rules from `2-plan-architecture-planning`, assign every identified module to exactly one layer. Verify domain modules have no adapter or runtime dependencies.

5. **Draw dependency edges:** For each scenario where the `Then` clause implies communication to a different module than the `When` clause acts on, draw a directed edge. Also draw edges where one module's output becomes another module's input, such as domain → ops or ops → adapter.

6. **Enforce acyclicity:** Walk the full graph. Detect and flag any cycle. A cycle is a hard blocker - resolve before proceeding.

7. **Enforce direction rules:** For each edge, verify it flows in the allowed direction per the architectural layer model. Flag any edge that crosses layers in the wrong direction.

8. **Name interface contracts:** For each edge that crosses a layer boundary, specify the interface contract: pseudocode function name, pseudocode message type, or channel description. Record which side owns the contract.

9. **Produce topological order:** List modules in the order they must be constructed, with leaves first and dependents last. This becomes the Stage 3 build order.

10. **Write dependency graph:** Emit structured markdown to `plans/<feature-slug>/plan/dependency-graph.md` with sections:
    - **Module Inventory:** module name, layer, description, owning entity/aggregate
    - **Dependency Graph:** text-format DAG (or ASCII diagram) showing all directed edges
    - **Interface Contracts:** for each layer-boundary edge: interface name, owner, pseudocode signature
    - **Topological Build Order:** ordered list from dependencies to dependents
    - **Architectural Decisions:** any placement choices that required judgment, and why

    Return the artifact path with a completion summary.

## Validation Checklist

Before emitting the graph:
1. ✓ Every domain entity and aggregate has a module placement
2. ✓ Every cross-module communication implied by behavioral scenarios has an edge
3. ✓ No cycles exist in the graph
4. ✓ All edges flow in the allowed direction
5. ✓ Every layer-boundary edge names an interface contract
6. ✓ Topological build order is valid (no forward dependencies)
7. ✓ Domain modules are free of adapter and runtime dependencies

## Handoff

**Success:** Emit the dependency graph file path and module count.

**Failure:** Report the specific cycle, direction violation, or missing
placement that blocks completion and include diagnostic guidance for the caller.
8. ✓ If existing actors from `system-actor-graph.yml` are included, no new
   proposed edge creates a cycle when combined with existing edges from
   that file