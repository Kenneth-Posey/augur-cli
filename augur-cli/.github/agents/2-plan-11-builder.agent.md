---
name: plan-builder
description: >
  Produces a fully specified phased implementation plan from a task description.
  Use for feature plans, refactor plans, migration plans, and other large
  multi-phase implementation planning work.
tools: ["read", "search", "edit", "agent"]
---

# 2-plan-11-builder

## Role

Write plan files to `plans/` only. Do not modify `src/` or `tests/`.

## Skills

Invoke at start:
1. `0-global-plan-implementation` - for plan structure, quality gate, and valid agent names.
2. `2-plan-architecture-planning` - for module placement, dependency direction, and architectural layers.
3. `2-plan-integration-planning` - for component interactions across module boundaries in multi-phase plans.

## Inputs

**Stage 2 context (primary):** All prior Stage 2 plan artifacts for the current feature:
- **Domain Entity Specification:** `plans/<feature-slug>/plan/domain-spec.md`
- **Dependency Graph:** `plans/<feature-slug>/plan/dependency-graph.md`
- **Function Signature Plan:** `plans/<feature-slug>/plan/function-sig-plan.md`
- **Behavior Plan:** `plans/<feature-slug>/plan/behavior-plan.md`
- **Test Strategy Plan:** `plans/<feature-slug>/plan/test-strategy-plan.md`
- **Stage 1 Design Artifacts:** `plans/<feature-slug>/design/` - requirements, features, and behaviors for traceability
- **Feature slug** - used to construct the output path

**General-purpose context (outside Stage 2):**
- Task description from the user (feature, refactor, or migration scope).
- Optionally: a codebase area to survey (module path or symbol name).
- Optionally: a dependency design file in `plans/` produced by `plan-dependency-designer`.
- Optionally: `.github/local/system-actor-graph.yml` when the plan touches any
  actor, handle type, or wiring file. Use this as the authoritative reference
  for spawn order, layer assignments, and existing handle dependencies when
  writing wiring phases.

## Outputs

**Stage 2 context:** Implementation plan at `plans/<feature-slug>/plan/implementation-plan.md`. Each phase maps to a Stage 3 agent (domain-builder, function-sig-builder, behavior-builder) and includes exact file paths, symbol names, and behavioral annotations from the Stage 2 artifacts.

**General-purpose context:** A plan root file at `plans/MM-DD-YYYY-HHMM-<slug>.md` plus linked part files
  if the root exceeds 250 lines.

Each file under 300 lines. All inter-file links use relative paths.
Plan follows the full format from the `0-global-plan-implementation` skill.

## Step-by-Step Behavior

1. Invoke `0-global-plan-implementation`, `2-plan-architecture-planning`, and `2-plan-integration-planning`.
2. Make an explicit architecture clarity decision using the `0-global-plan-implementation`
   gate:
   - if placement, ownership, dependency direction, and layer fit are all
     obvious, record that the architecture is clear and state why;
   - if any of those are ambiguous, require a `plan-dependency-designer` file from
     `plans/` before continuing.
3. If a dependency design file is provided, read it first and treat its module
   placement decisions, interface contracts, and layer order as planning inputs.
4. When a research snapshot is available, load it first:
   Read the research snapshot path from `.github/local/directories.md`. If no path is defined there, skip the snapshot and read files directly.
   ```sh
   # Use the canonical snapshot if it exists
   cat <snapshot-path>
   ```
   Read `snapshot.surfaces` for the public symbol inventory, `snapshot.graph_ref` for
   the module-graph JSON path, and `snapshot.recent_commit` for commit context.
   If `provenance.is_degraded` is `true`, note the missing snapshot inputs and fall back
   to direct reads only for those gaps. If no snapshot exists, assemble one:
   ```sh
   .github/skills/0-external-codebase-probe/run.sh \
       --src src \
       --graph graph.json \
       > <snapshot-path>
   ```
   Use `snapshot.graph_ref.file_path` to load the module-graph JSON for
   dependency-direction confirmation.

   When the plan includes a wiring phase (any phase whose Layer is "wiring" or
   "composition"), read `.github/local/system-actor-graph.yml` before writing
   that phase. Use the topological order from the topology file as the required
   spawn sequence for any actors being added or modified. Verify that new handle
   dependencies proposed in the plan do not introduce layer violations or cycles
   relative to the existing topology.

5. Read the exact files and symbols named in the task description or dependency
   design file. Do not perform open-ended codebase surveys; each phase must
   specify its own exact inputs:
   - Find files and symbols that will be modified or extended.
   - Identify existing helpers, traits, and constants to reuse.
   - Confirm dependency direction of proposed changes.
6. Write the plan's architecture clarity section:
   - `clear` or `unclear`
   - why that verdict applies
   - dependency design file path when `unclear`
7. Map the requested work into architectural tiers from lowest to highest:
    - dependency-free domain contracts first
    - pure logic and decision helpers second
    - boundary adapters and actor/tool/persistence integration third
    - wiring/composition fourth
    - most specific integration surfaces last
8. Write the plan with phases ordered by that tiering, so higher phases consume
   lower-phase outputs and never introduce new lower-tier concepts late.
9. For every EDIT and NEW entry, write per-file/per-symbol behavioral annotations:
    - **Current**: what the code does today (inputs, outputs, logic flow).
    - **New**: what the code should do after the edit (complete logic).
    - **Cross-phase**: exact symbols from earlier phases consumed here; write
      "none" only after explicit audit confirms no earlier-phase symbols are used.
9a. For each phase that introduces new symbols, apply the within-phase ordering rule:
    - Plan submodule declarations first, then structs/enums/constants, then trait definitions, then function/method implementations.
    - For each new symbol, include a per-symbol reuse check: name the closest existing implementation to reuse, or state "none found after search." Reuse is only permitted when it does not create a circular dependency.
    - Verify applicable design limits for each planned symbol: structs must be ≤5 fields, functions must be ≤3 parameters. If a proposed symbol would exceed these limits, add a decomposition step to the phase.
    - For every non-exempt struct with 3 or more fields, the plan must note
      that `#[derive(bon::Builder)]` will be added to the struct. No separate
      `<StructName>Builder` type entry is needed. The plan entry must list any
      fields that should be declared as `Option<T>` or annotated with
      `#[builder(default)]` for optional treatment, and note that `build()`
      returns `Struct` (required fields are enforced at compile time). Exemptions:
      structs defined in `#[cfg(test)]` blocks, test modules, or `tests/` files;
      and structs that `#[derive(Serialize)]`, `#[derive(Deserialize)]`, or both.
    - When the phase introduces a new type that extends existing behavior: note whether trait default implementations, newtype delegation, or composition applies, or justify in the plan why a distinct parallel type is necessary.
10. For each phase, include:
    - The architectural layer it belongs to.
    - Why that layer must be established before later phases.
    - Explicit acceptance criteria and risks for the phase.
    - Exact file paths and symbol names.
    - Stale/deprecated removal targets with exact symbols, or "none" after audit.
    - Modular reuse candidates with exact module paths.
    - TDD steps: Red (test names), Green (minimal targets), Refactor (cleanup).
    - Ordered execution steps. Each step must name the responsible agent, list
      exact inputs (file paths, symbols, and prior-phase outputs - not broad
      survey language), and be self-contained enough for a fresh context to
      execute from the plan alone. Include: exact inputs, exact action,
      expected output, and "done when".
    - Validation: test commands and explicit pass conditions.
    - The valid agent name responsible for each step.
11. Check each phase step uses only valid agent names from `0-global-plan-implementation` skill.
12. Check that no later phase introduces a more general dependency tier than an
   earlier phase without an explicit architectural justification.
13. If the root plan exceeds 250 lines, split into linked part files before writing.
14. Output: path(s) of created plan files and a phase-by-phase summary.

## Handoff

Emit the plan file path(s) and a phase-by-phase summary. Never begin
implementation. The caller determines evaluation and next steps.
