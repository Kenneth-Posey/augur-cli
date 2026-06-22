---
name: 0-global-plan-implementation
description: >
  Standards and templates for writing large multi-phase implementation plans.
  Use when creating, structuring, or reviewing phased implementation plans.
---

# Large Implementation Planning Standards

## Core Requirements

- Plans MUST be phase-based. Do not create single-block plans for large implementation work.
- Every phase MUST list exact implementation targets, including specific files and specific methods/functions/traits/structs that will be updated.
- Every phase MUST be executable by a fresh conversation context using only:
  the written plan, the current repository state, and the `.github/` rules and
  skills. Do not rely on unstated memory from earlier turns.
- Plans should sequence phases from the lowest architectural tier to the
  highest: start with the most general, dependency-free core logic and build
  upward toward adapters, wiring, and the most specific integration surfaces.
  Use this order unless the repository structure clearly requires otherwise;
  reviewers enforce final tier placement during implementation review.
- Each phase MUST include stale/deprecated code cleanup tasks where applicable. Name exact files and exact symbols to remove (traits, structs, enums, methods/functions, fields, modules).
- Each phase MUST include modular-reuse checks. Reuse existing calculations/utilities instead of duplicating formulas or logic across modules.
- Each phase MUST be TDD driven: Red -> Green -> Refactor.
- When review finds missing or partial plan work, the review MUST include required remediation suggestions mapped to specific phase requirements.
- When a follow-up item is created, it MUST be written as a new file in `plans/` using a date-time-prefixed filename and full implementation details.

## Key Files

- `README.md` - overview and usage notes

## Layered Fractal Planning Order (Mandatory)

Phases must respect a bottom-up dependency order. Prefer this sequence unless
the repository structure proves a different order is strictly necessary:

1. **Core domain layer** - semantic newtypes, shared domain structs/enums,
   constants, traits, and dependency-free contracts.
2. **Pure logic layer** - `_ops` module helpers, calculations, policy functions,
   decision enums, parsers, and pure state-transition helpers.
3. **Boundary adapter layer** - actors, tool handlers, persistence adapters,
   config loaders, and other modules that translate between the outside world
   and the core logic.
4. **Wiring and composition layer** - composition root module, handle assembly,
   feed/channel hookup, construction order, and dependency injection.
5. **Most specific integration layer** - UI/TUI surfaces, command entrypoints,
   final end-to-end coordination, and docs/changelog updates tied to the
   completed behavior.

## Techniques For Enabling Fractal Architecture

The plan must explicitly describe how each phase preserves the same shape at
multiple scales: general core below, specific orchestration above.

- **Push decisions downward**: define decision enums, policies, and calculations
  in the lowest layer that can own them; keep upper layers responsible for
  side effects and coordination only.
- **Pull side effects upward**: if a helper needs I/O, channels, logging, async,
  or runtime handles, keep that behavior in the adapter layer and pass plain
  data into lower layers.
- **Stabilize contracts first**: create or update newtypes, traits, structs,
  constants, and decision enums before planning phases that consume them.
- **Compose upward**: later phases should assemble previously introduced domain
  units into larger structures rather than introducing new low-level concepts
  late in the plan.
- **Test by layer**: core and pure logic layers lead with focused unit tests;
  adapter and wiring layers add actor/integration tests after the lower-layer
  contracts already exist.
- **Name each layer in the phase objective** when the phase establishes new
  lowest-tier contracts or builds on prior tiers.

### Within-Phase Symbol Introduction Order

Within each phase, plan new symbols in this order:

1. Submodule declarations
2. Structs, enums, and constants
3. Trait definitions
4. Functions and method implementations

Use this sequence as planning guidance so types appear before dependent traits
and functions. It is not a hard gate.

For each new symbol in the phase, the Modular Reuse Audit entry must be per-symbol: name the closest existing candidate to reuse, or state "none found after search." The phase-level sweep alone is not sufficient; each symbol needs its own entry.

When a phase introduces a new type that extends or resembles an existing type, the plan must note whether composition, delegation, trait default implementations, or the newtype pattern applies, or justify why a distinct parallel type is necessary.

When verifying size limits, apply the size limits from `0-global-line-count-check` to each planned struct (≤5 fields) and function (≤3 parameters). Flag planned symbols that would violate these limits before writing the phase to file.

## Architecture Clarity Gate (Mandatory)

Before writing the plan, the planner MUST decide whether the architecture is
**clear** or **unclear** and record that decision in the plan or its
prerequisite architecture skeleton.

### Architecture Is Clear Only If ALL Conditions Hold

1. **Placement is obvious**: the exact target layer and module path are already
   evident from existing repository structure and patterns.
2. **Ownership is obvious**: it is already clear which actor, domain module,
   adapter, or boundary owns the new state, decisions, and outputs.
3. **Dependency direction is obvious**: the change can be added without
   uncertainty about import direction, layering order, or cycle risk.
4. **Layer fit is obvious**: it is clear which parts belong in core domain,
   pure logic, boundary adapters, wiring, and the most specific integration
   layer.
5. **Contracts already exist or are trivial to place**: any needed newtypes,
   traits, structs, decision enums, or helper contracts have an obvious lowest
   layer where they belong.
6. **No competing placements**: there is not more than one plausible home for a
   major piece of logic, state ownership, or boundary translation.

### Architecture Is Unclear If ANY Condition Holds

1. The feature spans more than one domain, actor family, or major subsystem.
2. New boundaries, new modules, or new ownership surfaces must be introduced.
3. State ownership, feed ownership, or decision ownership is ambiguous.
4. More than one architectural layer is a plausible home for key logic.
5. New low-level contracts and higher-level adapters would need to be invented
   together without an already obvious separation.
6. A dependency cycle, upward reference, or wrong-direction import seems
   possible.
7. The work may require changing component handles, `_ops` module placement, wiring, or
   shared domain contracts in a way not already established by nearby code.
8. The planner cannot explain, in one sentence per major symbol, why that symbol
   belongs in its proposed layer.

### Required Action From The Clarity Decision

- If architecture is **clear**:
  - the plan MUST include a short written justification explaining why the
    architecture is clear enough to proceed without `plan-dependency-designer`.
- If architecture is **unclear**:
  - `plan-dependency-designer` MUST run first and write an architecture skeleton to
    `plans/`;
  - `plan-builder` MUST consume that file before writing the implementation
    plan;
  - the implementation plan MUST reference the architecture file it builds on.

## Control-Boundary Note

Plans may describe phase-local work, exact inputs, expected outputs, and local
validation bars. Do not embed checkpoint routing, handoff graphs, retries, or
next-phase control text; orchestration surfaces own those behaviors.

## Behavioral Edit Annotations (Mandatory)

Every EDIT and NEW entry in a phase MUST include per-file, per-symbol annotations:

1. **Current behavior** (for EDITs): What the existing code does today, in concrete terms
   (inputs, outputs, logic flow, side effects). One annotation per file/symbol pair.
2. **New behavior**: What the code should do after the edit. Describe complete logic.
   One annotation per file/symbol pair.
3. **Cross-phase dependencies** (every entry): Which earlier phase produced the
   function, type, or module being consumed. Name the exact symbol and the phase.
   Write "none" only after explicit audit confirms no earlier-phase symbols are used.
4. **Strategy** (EDIT entries): `add-replace | direct-edit`. If `direct-edit` is
   used, include explicit justification.

## Plan File Size and Linking (Mandatory)

- Each plan file MUST NOT exceed 300 lines (hard `wc -l` limit).
- When a plan exceeds 300 lines, split into a root plan file and linked part files.
- Root plan: overall goal, scope, phase index with relative links, verification matrix.
- Part files: subset of phases; open with `# Implementation Plan - Part N of M`. Do not include Root/Previous/Next navigation links.
- All links between plan files MUST use relative paths within the same directory.

## Follow-Up File Standard (Mandatory)

Follow-up filenames: `MM-DD-YYYY-HHMM-<followup-slug>.md`. Each file must include:
problem statement, affected phases/files/symbols, required behavior changes,
constraints, TDD expectations, validation commands, cleanup requirements, risk notes.

## Required Plan Format

1. Goal and Scope - problem statement and non-goals.
2. Architecture Clarity Decision - clear vs unclear, justification, and
   dependency-designer file reference when required.
3. Phase Breakdown - phase name, objective, acceptance criteria, risks.
4. Layering Strategy - architectural tier order and phase-to-layer mapping.
5. Per-Phase Implementation Map - exact files, exact symbols, behavioral annotations.
6. Per-Phase Execution Steps - ordered, role-owned, self-contained actions.
7. Per-Phase Stale Code Removal - deprecated/duplicate symbols to delete.
8. Per-Phase Modular Reuse Audit - existing helpers to reuse, dedup opportunities.
9. Verification Matrix - test files per phase, expected Red/Green states.
10. Phase Completion Checklist - all gates before marking the phase work ready.

## Per-Phase Template

```
- Phase: <name>
- Objective: <single clear objective>
- Layer: <core domain / pure logic / boundary adapter / wiring / integration>
- Why this layer now: <which lower-tier contracts are established here before higher tiers depend on them>
- Risks: <explicit risks for this phase; what can go wrong and what conditions cause failure>
- Files and Symbols:
  - File: <path> - Symbols: <method/function/trait/struct/enum/field names>;
    Strategy: add-replace | direct-edit - <justification if direct-edit>;
    Current: <what this code does today, in concrete terms>;
    New: <what the code should do after the edit, with complete logic>;
    Cross-phase: <exact symbols from earlier phases consumed here, or none>
- TTD/TDD Steps:
  - Red: [exact test IDs that must fail first]
  - Green: [pass condition - omit if standard cargo nextest run]
  - Refactor: [specific extraction target, or omit if none]
- Execution Steps:
  - Step: <ordered step number>
  - Role: <responsible role from approved role list>
  - Inputs: <exact files, exact symbols, and exact prior-phase outputs this step needs>
  - Action: <specific change or analysis to perform>
  - Output: <artifact, code path, or decision produced by this step>
  - Done when: <observable completion condition>- When a phase Layer is "wiring" or "composition", include an explicit step
    to update `.github/local/system-actor-graph.yml` if any actors are added,
    removed, or rewired. Assign this step to `utility-topology-extractor` for
    full regeneration or include it as a manual file edit step for small changes.
- Stale/Deprecated Removal:
  - Remove: <exact symbols and files; displaced functions from add-replace are primary targets; write "none" only after explicit audit>
- Modular Reuse:
  - Reuse: <existing modules/helpers/constants>
- Validation:
  - Tests: <exact files/commands>
```

## Planning Quality Gate

A plan is incomplete unless:

- Split into explicit phases.
- The plan explicitly records an architecture clarity decision and follows the
  required action for that decision.
- Phases should be ordered from lowest architectural tier to highest as a
  guide; note deviations, but do not fail the plan on tier placement alone
  because review enforces final placement correctness.
- Every phase names exact files and exact symbols.
- Every phase contains explicit risks. Phase-specific acceptance criteria are included in the Validation section only when they differ from the standard validation command.
- Every EDIT/NEW entry for each file has per-file/per-symbol behavioral
  annotation: Current (concrete description of today's behavior), New (complete
  target logic), and Cross-phase (exact symbols from earlier phases consumed, or
  "none" after explicit audit). Every EDIT entry also includes Strategy:
  `add-replace | direct-edit`, and `direct-edit` includes explicit
  justification; plan-evaluator fails direct-edit entries without justification.
- Every execution step names exact inputs (exact file paths, exact symbol names,
  exact prior-phase output references) - broad survey language is not accepted.
- Every phase contains ordered execution steps that are self-contained enough
  for a fresh context to execute without relying on prior conversation memory.
- Every phase is TDD Red-Green-Refactor.
- Every phase contains stale/deprecated removal with exact targets or an
  explicit "none" after audit.
- Every phase contains modular reuse and dedup checks.
- No single plan file exceeds 300 lines.
- Every execution role in phase steps is from the approved role list below.
- Within each phase, new symbols should follow the within-phase introduction
  order (submodules → structs/enums/constants → traits → functions) as
  planning guidance, not a hard gate.
- The Modular Reuse Audit entry is per-symbol: each new constant, struct, enum,
  trait, or function names a reuse candidate or states "none found after
  search."
- Each new struct planned in a phase is ≤5 fields. Each new function is ≤3
  parameters. Symbols that would exceed these limits must include a
  decomposition plan for that phase.
- Any new type that substantially mirrors an existing type includes a written
  justification for why composition, delegation, or trait-based extension was
  not used.

## Valid Role Names

Use only these names when referencing execution roles in plan phase steps.

### Pipeline-canonical roles

- design-requirements-builder
- design-requirements-reviewer
- design-features-builder
- design-features-reviewer
- design-behavior-builder
- design-behavior-reviewer
- plan-domain-designer
- plan-domain-reviewer
- plan-dependency-designer
- plan-dependency-plan-evaluator
- plan-function-sig-planner
- plan-function-sig-reviewer
- plan-behavior-planner
- plan-behavior-plan-reviewer
- plan-test-planner
- plan-test-reviewer
- plan-builder
- plan-evaluator
- plan-gap-analyst
- implement-domain-builder
- implement-domain-reviewer
- implement-function-sig-builder
- implement-function-sig-reviewer
- implement-test-author
- implement-test-tdd-reviewer
- implement-behavior-builder
- implement-behavior-implementation-reviewer
- review-architecture-checker
- review-behavior-checker
- review-activation-checker
- review-type-checker
- review-function-sig-checker
- review-performance-checker
- review-security-checker
- review-consistency-checker
- review-completeness-checker
- review-consolidation-checker
- review-consolidator
- external-code-stub-detector
- global-writer-changelog
- global-git-operator
- utility-quick-patch-design
- utility-quick-patch-plan
- utility-quick-patch-code
- utility-quick-patch-tests

### Auxiliary roles (non-pipeline work)

- design-orchestrator
- plan-orchestrator
- implement-orchestrator
- review-orchestrator
- global-session-resume-orchestrator
- global-pipeline-orchestrator
- utility-code-newtype-migrator
- utility-code-rust-implementer
- utility-code-refactorer
- global-code-reviewer
- external-code-tool-analyst
- external-code-src-deadcode-analysis
- external-code-actor-ops-detector
- external-code-rustc-dependency-check
- global-customization-author
- global-customization-reviewer
- utility-doc-author
- utility-question-answering
- utility-topology-extractor
- global-triage-failure
