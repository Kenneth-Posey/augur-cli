---
name: 4-review-behavior-validation
description: >
  Stage 4 behavior validation contract. Verify test execution, coverage,
  panic-safety, and feature completeness independent of language before
  integration testing.
---

# Skill: 4-Review Behavior Validation

## Purpose

Validate that the implementation satisfies behavioral requirements: tests pass,
coverage meets the target, production library code avoids unjustified panic
patterns, and every planned feature has a passing test.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

### 1. Test Execution
- All unit tests pass (exit code 0)
- All integration tests pass
- All documentation-embedded tests compile and execute successfully
- No test panics, hangs, or timeouts

### 2. Code Coverage
- Line (and branch where available) coverage meets or exceeds the target threshold
  (default: 80% unless the plan specifies otherwise)
- Coverage report is generated
- Uncovered lines are justified in comments or accepted by the plan

> **Coverage Matrix:** The behavior-report is the authoritative source for the
> BEH-ID → test function mapping table. Emit the full matrix here. Other
> checkers (completeness, consistency) reference this report rather than
> reproducing the table.

### 3. Library Code Panic Safety
- Production library/core code contains no unconditional panic patterns
  (`unwrap`, `expect`, `panic!`, unchecked indexing without justification)
- Errors are handled explicitly rather than through panics
- Test code and binary entry points are exempt

### 4. Feature Completeness
- Every feature in the Stage 2 behavioral specification is implemented, not stubbed
- Every feature has at least one corresponding passing test
- Feature flags are declared and tested

## Pass Conditions

- All test suites execute with exit code 0
- Coverage ≥ target threshold
- No unjustified panic patterns in production library code
- All planned features have passing tests

## Fail Conditions

- **Critical:** Any test fails (non-zero exit code)
- **Critical:** Coverage below target by more than 5 percentage points
- **Critical:** Any panic or hang during test execution
- **High:** Coverage below target by 1–5 percentage points
- **High:** Panic pattern found in library code without justification
- **High:** Planned feature has no corresponding test

## Validation Signal

| Severity present | Signal |
|---|---|
| Critical or High findings | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |

## Report Format

**On pass (signal = pass):**
- Emit one summary line per validation category in the form:
  `Category Name: ✓ (brief note, e.g., "12 modules verified")`
- Emit the JSON diagnostic block with `findings: []` (or `findings` with only
  Medium/Low entries if present)
- Omit: detailed row-by-row verification tables, per-item bullet lists,
  validation checklists, and any duplicate `## Signal` section at the bottom
  - the signal is already stated in the report header

**On fail (signal = fail):**
- Emit full detail (table/bullets/evidence) only for the failing categories
- Emit the summary line format for all passing categories
- Emit the JSON diagnostic block with all findings fully populated

## Language Companion

Look up `4-review-behavior-validation` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for the
language-specific test commands, coverage tool invocation, panic-pattern detection
rules, and checker logic.
