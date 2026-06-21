---
name: 4-review-function-sig-validation
description: >
  Stage 4 function signature validation contract. Checks function coverage,
  type correctness, ownership/reference semantics, error handling,
  generic/interface bounds, and signature documentation against the plan,
  independent of language.
---

# Skill: 4-Review Function Signature Validation

## Purpose

Validate that each implemented function signature matches its plan: parameter
types, return types, error types, ownership semantics, generic/trait bounds,
and documentation. This skill covers signatures only; behavior validation
covers function bodies.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

### 1. Function Coverage
- Every function listed in the Function Signature Plan has a corresponding
  implementation
- No extra functions are introduced without a plan item
- All function names match the plan exactly (casing, spelling)

### 2. Type Signatures
- Parameter types match the plan exactly
- Return types match the plan exactly
- Error/exception types match the plan exactly
- No implicit type coercions or widening that deviates from the plan

### 3. Ownership and Reference Semantics
- Ownership transfer, borrowing, or copying match the intended semantics for
  each parameter
- Mutable parameters are used only when the function modifies the value
- Reference lifetimes (where the language exposes them) are correct and justified

### 4. Error Handling Completeness
- Functions that can fail use the language's idiomatic error propagation
  mechanism (not panicking or swallowing errors)
- Error types declare all plan-specified failure variants
- Infallible functions do not wrap their return type unnecessarily

### 5. Generic and Interface Bounds
- All generic type parameters carry required bounds
- Bounds are sufficient for the function body's usage without over-constraining
- Interface/trait object bounds are complete and correct

### 6. Parameter Count
- Parameter lists do not exceed the project maximum (default: 3 parameters;
  use a parameter object for larger groupings)

### 7. Signature Documentation
- Every public function's doc comment covers parameters, return value, error
  variants, and pre/post-conditions

## Pass Conditions

- All plan functions are present with exactly matching signatures
- Ownership semantics match domain intent
- Error handling is complete and idiomatic
- Bounds are sound and non-redundant
- All public signatures are documented

## Fail Conditions

- **Critical:** Function present in plan but missing from implementation
- **Critical:** Type mismatch between plan and implementation
- **Critical:** Incorrect ownership semantics (e.g., consuming where borrowing intended)
- **Critical:** Missing error variant required by plan
- **High:** Extra function not in plan
- **High:** Oversized parameter list (exceeds project maximum)
- **High:** Error type is overly generic (e.g., opaque error box instead of a typed enum)
- **Medium:** Missing signature documentation

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

Look up `4-review-function-sig-validation` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for the
language-specific lifetime annotation rules, error type conventions, trait bound
requirements, visibility semantics, and checker logic.
