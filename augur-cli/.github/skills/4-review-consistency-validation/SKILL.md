---
name: 4-review-consistency-validation
description: >
  Stage 4 consistency review. Verify naming, documentation, behavior, scope,
  and style across languages so the implementation matches its documented
  contracts.
---

# Skill: 4-Review Consistency Validation

## Purpose

Check that naming, documentation, code style, and behavior stay consistent
throughout the implementation. Public code should be documented, names should
follow project conventions, documented behavior should match the code, and
nothing should be out of scope.

## Key Files

- `README.md` - overview and usage notes

## What to Validate (Language-Agnostic Contract)

> **Coverage Matrix:** Do not reproduce the BEH→test mapping table. It is
> owned by the behavior-checker. State: "Full BEH→test mapping: see
> reports/behavior-report.md" and report only the pass/fail count summary.

### 1. Naming Conventions
- Module, function, and variable names follow the language's standard casing rules
- Type names (classes, structs, enums, interfaces) follow the language's type
  casing convention
- Constants and immutable globals follow the language's constant naming convention
- No deviations from the project's naming standards

### 2. Documentation Completeness
- Every public function has a documentation comment
- Every public type has a documentation comment
- Every public module or namespace has a documentation comment
- All public function parameters and return types are documented

### 3. Behavior-to-Code Alignment
- Each function's implementation matches its documented behavior (error types,
  return values, side effects)
- Each behavior's code path matches the Given/When/Then specification
- No undocumented side effects in public functions

### 4. Scope Integrity
- No scope creep: no code present that is not in the plan
- No plan gaps: no planned item is absent from the code
- Error variants or exception types are used correctly and match the expected types

### 5. Documentation Examples
- Any embedded documentation examples compile without errors
- Examples demonstrate correct usage of the function or type

### 6. Code Style
- Indentation and whitespace are consistent throughout the codebase
- Line length does not exceed the project maximum

## Pass Conditions

- Naming is uniform and follows project conventions throughout
- All public API items are documented
- Every implementation matches its documented contract
- No scope discrepancies (no creep, no gaps)
- All documentation examples are accurate and compile

## Fail Conditions

- **Critical:** Contract violation (implementation contradicts documented behavior)
- **Critical:** Missing specification implementation
- **High:** Missing documentation on a public API item
- **High:** Scope discrepancy (creep or gap)
- **High:** Behavior spec misalignment (code path does not match Given/When/Then)
- **Medium:** Naming convention violation
- **Medium:** Missing parameter or return type documentation
- **Medium:** Incorrect or outdated documentation example
- **Low:** Style inconsistency (indentation, line length)

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

See `4-review-consistency-validation` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for
language-specific naming rules, documentation formats, style standards, and
checker logic.
