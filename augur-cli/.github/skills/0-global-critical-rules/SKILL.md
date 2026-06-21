---
name: 0-global-critical-rules
description: >
  Safety, workflow, commit, and implementation completeness rules. Use at
  the start of any task to verify compliance with non-negotiable constraints.
---

# Critical Rules

## Before Starting Any Task

- Reference [`.github/local/rules.md`](../../local/rules.md) for project-specific
  rules on commits, branching, delegation, and implementation standards.
- Use `.github/copilot-instructions.md` for primary-context routing and
  workflow routing rules.
- Ask clarifying questions only when requirements, scope, or behavior are
  genuinely ambiguous. Once the task is clear enough to execute, implement
  immediately.
- Delegate to the appropriate specialized skill early. Use review skills for audits,
  `utility-question-answering` for broad repository questions, and
  `utility-quick-patch-code` or `utility-code-rust-implementer` for code changes.

## Commit Policy

- Small or non-phased changes: ALWAYS ask for user confirmation before committing.
- Large phased implementations: follow the repository's orchestration entrypoints
  for phase order, retries, and checkpoint handling instead of restating that
  flow locally.
- If a commit is explicitly requested by the user or by repository policy, route
  it through `global-git-operator`.

## Definition of Done

A task is done only when ALL of the following are true:

1. Tests are written first (TDD Red) and passing (TDD Green).
2. Implementation satisfies all tests.
3. Code is refactored for clarity (TDD Refactor).
4. Local tests pass (`cargo test --quiet`).
5. Acceptance criteria from plan are met.
6. Implementation is fully feature-complete for the requested scope.
7. No deferred implementations remain, and no requested-scope stubs or
   placeholders remain.
8. All reviewer and evaluator signals are binary: `pass` (100% requirements met)
   or `fail` (any gap). No "pass with notes." Passes require full compliance.

## Temporary Compile-Target Stubs vs Deferred Implementations

If you create domain types or function signatures before Red, only add the
minimal scaffolding needed for tests to compile. Those stubs are temporary, do
not satisfy Green, and must be completed during the Green cycle.

Do not leave stubbed or deferred behavior for requested features. No:

- Placeholder returns
- No-op branches
- TODO-later paths
- Temporary mock logic
- Partially wired code
- Compile-target stubs that remain after Green or at completion

Unless the user explicitly requests staged delivery.

## Test-First Development (Red-Green-Refactor)

All development follows TDD:

1. Red: Write a failing test that describes the desired behavior.
2. Green: Write minimal code to make the test pass.
3. Refactor: Clean up without changing behavior.

If tests need compile targets before Red, keep that scaffolding limited to the
domain types and function signatures required for compilation. It is not
behavior implementation, and all stubbed production bodies must be completed in
Green.

For bugs: write a failing regression test BEFORE fixing the code. The test must
fail without the fix and pass with it.

## SOLID and Modular Design

- Follow SOLID principles and DRY at all times.
- Eliminate duplication by extracting shared patterns into reusable helpers.
- Keep modules small and composable.
- When a file exceeds 200 lines of logic, use the `0-global-line-count-check` skill and
  refactor into smaller modular parts.
- When a function handles multiple concerns, split it into focused helpers.

## Add-Replace Update Strategy

- Default to add-replace when changing behavior in functions that already have callers:
  create a new function instead of modifying the existing one.
- Wire callers to the new function, then schedule the displaced function for
  stale-code removal.
- Direct edits to an existing function are acceptable only when the function has
  no callers, or when the change is a pure rename/signature fix with no
  behavioral difference.
- When replacing functionality represented by `if/else` or `match`, after adding
  the new branch reference, remove the legacy branch reference in the same
  scoped change unless the phase is explicitly scaffold-only.

## Run Tests After Implementation

After implementing a feature, run relevant local tests to verify with
`cargo test --quiet`. Do NOT run integration tests or Docker-based tests -
those are run by the user.

## External Tools

This skill uses the following external tools:

- [`0-external-syn-analyzer`](../0-external-syn-analyzer/SKILL.md) - AST-based Rust code quality analyzer for parameter counts, complexity, magic literals, missing docs, and more
- [`0-external-test-gap-fusion`](../0-external-test-gap-fusion/SKILL.md) - Combine mirror mapping, coverage data, pipeline results, and duplicate signals into a prioritized test-gap report
