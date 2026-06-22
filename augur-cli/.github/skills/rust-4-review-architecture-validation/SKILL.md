---
name: rust-4-review-architecture-validation
description: >
  Module placement, dependency direction, ownership boundaries, and architectural
  layer validation for the Rust codebase. Use during code review to ensure changes
  respect single-direction dependency flow, actor-domain ownership rules, and
  correct type visibility semantics.
---

# Rust 4 Review Architecture Validation

**Authority boundary**: Architecture and dependency direction only. Do not use
this skill for type correctness, behavior, performance, naming, or style review.

## Review Role

Use this skill to review Rust architecture concerns in scoped changes. Read the
changed modules, repo-local authorities, and any deterministic architecture
artifacts together, then emit the shared `pass|fail` signal.

## Key Files

- `README.md` - overview and usage notes

## Scope

### What This Skill Validates

1. **Module Placement**
   - New modules are placed in correct tier (`domain/`, `actors/`, `tools/`, `config/`)
   - No cross-tier misplacement (e.g., business logic in `actors/`, adapters in `domain/`)
   - `_ops.rs` modules (pure logic) are kept separate from actor shells
   - Assistant modules are placed correctly relative to their consuming actor

2. **Dependency Direction**
   - All `use` statements respect allowed direction per layer
   - `domain/` and `_ops.rs` modules do not import from `actors/`
   - `domain/` modules do not import from `config/`, `tools/`, or runtime modules
   - Actors import from domain correctly; domain never imports from actors
   - No circular imports (A → B → A)

3. **Ownership Boundaries**
   - Actor files own async execution, state, and channels
   - Pure `_ops.rs` modules handle business logic without actor context
   - Channel types and runtime handles stay at adapter boundaries
   - Domain types are runtime-agnostic

4. **Feed and Wiring Graph**
   - Actor-to-actor feeds form a directed acyclic graph (DAG)
   - No bidirectional feeds or circular actor subscriptions
   - Actor spawn order in `wiring.rs` respects dependency topological sort
   - Each actor has defined upstream sources and downstream outputs

### Coverage Boundaries

This skill assumes:
- All code compiles without errors
- The project structure follows `.github/local/directories.md` for layout and the
  relevant Stage 2 design artifacts - especially
  `plans/<feature-slug>/plan/dependency-graph.md`,
  `plans/<feature-slug>/plan/domain-spec.md`, and
  `plans/<feature-slug>/design/behaviors.md` - for architectural intent and
  dependency direction
- Module boundaries are already established (not designing new architecture)
- Validation scope comes from the review input, such as a scoped changed-file
  or module list, current deterministic tool output, or other repo-local review
  evidence

## Validation Process

### Validation Inputs

1. **Gather Review Input**: Start from the available review input:
   - Scoped changed-file list or module list when provided
   - Module paths to validate
   - Optional: specific architecture rules to emphasize
   - Current deterministic tool output when already available

2. **Scan Dependencies**: Use current deterministic `arch-linter` output when it
   is part of the review handoff. If fresh evidence is needed, run the
   project-approved `arch-linter` command for this repo. Extract findings for
   `boundary-contract`, `wrong-direction`, and `cycle` issues.

3. **Verify Module Placement**: For each new or modified module:
    - Check the file path matches expected tier
    - Compare against `.github/local/directories.md` for placement conventions,
      `plans/<feature-slug>/plan/dependency-graph.md` for intended boundaries, and
      `plans/<feature-slug>/plan/domain-spec.md` when ownership placement is relevant
    - Flag misplaced modules (e.g., business logic in `actors/`)

4. **Check Dependency Edges**: For each `use` statement in changed files:
   - Verify the edge follows allowed direction per layer
   - Flag reverse edges (domain → adapters, pure → actors)
    - Detect cycles using current deterministic `module-graph` output or the
      project-approved `module-graph` command for this repo

5. **Validate Actor Decomposition**: For actor files:
   - Confirm actor shell and `_ops.rs` core are separate
   - Verify `_ops.rs` has no actor/channel dependencies
   - Check assistant modules are clearly named and bounded relative to the
     ownership and interaction expectations in `plans/<feature-slug>/plan/domain-spec.md`
     and `plans/<feature-slug>/design/behaviors.md`

6. **Report Findings**: Output violations with severity:
   - Critical: circular dependencies
   - Major: reverse-direction dependencies (wrong way)
   - Minor: misplaced modules or potential future violations

7. **Read-Only Review**: Record findings against the changed files and governing
   artifacts so follow-up work can update code or the cited files as needed.

## Architecture Reference

For detailed rules, see:

- **Module Layering**: See `.github/local/directories.md` for source-tree, test,
  and placement conventions; use `plans/<feature-slug>/plan/dependency-graph.md`
  as the primary authority for intended module placement
- **Dependency Direction**: See `plans/<feature-slug>/plan/dependency-graph.md`
  together with `plans/<feature-slug>/plan/domain-spec.md` for allowed layer
  crossings and ownership semantics
- **Ownership and Decomposition**: See `plans/<feature-slug>/plan/domain-spec.md`
  for ownership boundaries and `plans/<feature-slug>/design/behaviors.md` for
  scenario-implied feed/wiring expectations
- **Ports and Adapters**: See the interface contracts and boundary crossings
  recorded in `plans/<feature-slug>/plan/dependency-graph.md` and
  `plans/<feature-slug>/design/behaviors.md`

## Validation Signal

Use the same `pass|fail` vocabulary as the deterministic architecture
tools.

| Condition | Signal |
|----------|--------|
| Critical architecture break or repeated major boundary violations | `fail` |
| Minor-only drift or documented exceptions that remain non-blocking | `pass` with warnings |
| Validation timed out or required evidence is incomplete | `fail` |

## Deterministic Tool Inputs

Use these tool outputs only when they are part of the review handoff or are
re-run deterministically for the current tree:

1. **arch-linter**
   - Detects boundary and direction violations; use first for primary findings

2. **module-graph**
   - Use to confirm cycles and trace edges

3. **dependency-intel**
   - Use for cross-crate dependency issues

## Key Principles

1. **Domain Never Depends on Adapters**: Preserves reusability and testability
2. **Actors Depend on Domain**: One-way dependency from boundary to core
3. **Feed Graph is a DAG**: Prevents deadlocks and circular reasoning
4. **Pure Logic Separate from Async**: Improves testing and unit-testability
5. **Clear Module Boundaries**: Aids understanding and prevents leakage

## Open Questions and Required Follow-Up

If validation finds:
- **Ambiguous layer membership**: Mark the review blocked until
  `plans/<feature-slug>/plan/dependency-graph.md` or
  `plans/<feature-slug>/plan/domain-spec.md` is clarified. Do not infer a new
  layer assignment during review.
- **Intended architecture exceptions**: Require an explicit update to
  `plans/<feature-slug>/plan/dependency-graph.md` and, when the exception changes
  visible behavior, `plans/<feature-slug>/design/behaviors.md`, then re-validate.
- **Cross-cutting concerns**: Record the missing accommodation in
  `plans/<feature-slug>/plan/implementation-plan.md` plus any affected
  architecture handoff file before approval.

If the architecture has violations, emit `fail` and record the specific violations found.
