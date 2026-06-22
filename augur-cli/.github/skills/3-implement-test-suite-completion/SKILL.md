---
name: 3-implement-test-suite-completion
description: "Closes Stage 3 test-suite coverage gaps in language-neutral terms while preserving Red/Green discipline, validating real behavior, and requiring all planned tests to pass with no production stubs left. Use at Stage 3 to implement or finish tests from the test plan."
---

# Skill: 3-implement-test-suite-completion

## When to Use

Use this skill when Stage 2 test planning has defined scenario coverage, failure cases, and pass conditions, and Stage 3 must implement the missing tests and prove Green without shortcuts.

Before choosing test runner commands, naming syntax, file placement, or framework mechanics, consult [`.github/local/language-companions.md`](../../local/language-companions.md) for the language-specific companion for the current stack.

## Key Files

- `README.md` - overview and usage notes

## Inputs and Dependencies

- Test plan from `2-plan-test-planning`
- Behavior plans from `2-plan-behavior-planning`
- Signature plans from `2-plan-function-sig-planning`
- TDD discipline from `0-global-tdd-workflow`

## Stage 3 Guardrails

1. **Tests come from the plan, not from what seems convenient during Green.**
2. **Red must be real.** Tests should fail for the intended missing behavior before the implementation is completed.
3. **Compile-target stubs are allowed only before Red.** If a missing symbol prevents the first failing test from compiling, add the thinnest temporary stub needed to reach that failing test, then replace it immediately.
4. **Green requires full planned coverage.** “Some tests pass” is not enough; all planned tests for the current scope must pass or be explicitly deferred in the plan.
5. **Green also requires zero production stubs.** No placeholder implementation, fake-success branch, or temporary no-op may remain in production code once the test suite is complete.

## Workflow

### 1. Perform gap analysis against the plan

For each planned scenario, determine whether a corresponding test already exists and covers:

- the planned trigger
- the planned success outcome
- the planned failure outcome
- the relevant edge or state case

If coverage is partial, the gap still exists.

### 2. Implement tests in Red-first order

Add or complete tests so that each new test first demonstrates the missing behavior. Keep the failure mode obvious:

- the assertion should fail for the intended reason
- the setup should isolate one behavior path
- the test should not depend on hidden shared state

### 3. Cover all planned categories for the scope

Use the test types already chosen in the plan:

- focused unit tests for isolated rules
- integration tests for cross-boundary behavior
- property-style or generated tests for invariant-heavy logic
- performance or regression tests only when the plan requires them

Do not substitute one category for another just because it is easier to write.

### 4. Verify real behavior, not internal implementation trivia

Tests should prove the observable contract:

- returned values
- state transitions
- persisted effects
- emitted messages or integration outcomes

Avoid brittle tests that only confirm internal helper calls or incidental structure.

### 5. Close Green only when complete

The implementation is complete only when:

- every planned test in scope passes
- full required regression checks pass for the scope
- no production compile-target stub remains
- no placeholder branch is still carrying real behavior paths

If a test is still missing, still flaky, or still skipped without a plan-approved deferral, the suite is not complete.

## Validation Checklist

- [ ] Every planned scenario in scope is mapped to a real test or explicit plan-approved deferral
- [ ] New tests fail meaningfully before the implementation change that satisfies them
- [ ] Tests cover planned success, failure, and edge/state scenarios for the scope
- [ ] Tests assert observable behavior rather than incidental implementation details
- [ ] Full required test execution for the scope passes
- [ ] Any temporary compile-target stub used before Red has been removed or replaced
- [ ] No production placeholder behavior remains anywhere in the code path under test

## Relationship to Other Stage 3 Skills

- `3-implement-domain-implementation` provides the domain behavior and invariants the tests must prove
- `3-implement-function-sig-implementation` provides the contract surfaces the tests invoke
- `3-implement-behavior-wiring` provides the end-to-end flow that integration and behavior tests validate
