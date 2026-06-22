---
name: 4-review-type-validation
description: >
  Stage 4 type-validation contract. Verify lifetime and ownership correctness,
  generic bounds, unsafe justification, and semantic type usage across
  languages.
---

# Skill: 4-Review Type Validation

## Purpose

Validate that the type system is used correctly: references do not outlive
their values, generic bounds are necessary and sufficient, unsafe operations
are justified, and semantic types are used instead of bare primitives.

## Key Files

- `README.md` - overview and usage notes

## What to Validate

### 1. Lifetime and Ownership Correctness
- No dangling references or use-after-free patterns
- Where the language exposes lifetime annotations, they are present and correct
- Output lifetimes are traceable to input parameters or the static lifetime
- Variance rules are respected where applicable

### 2. Generic Type Bounds Soundness
- All generic type parameters carry required bounds
- Bounds are sufficient for the usage within the function or data structure body
- No unnecessary or over-constraining bounds that restrict the API without benefit
- Interface/trait object bounds are complete and include required lifetime markers

### 3. Unsafe Operation Justification
- Every unsafe region has an inline comment documenting the invariants that
  make it safe
- Safety requirements are specific and verifiable
- Safer alternatives have been ruled out before using unsafe code

### 4. Semantic Type Usage
- Domain concepts with meaningful constraints (IDs, handles, validated strings,
  bounded numerics) use distinct wrapper/newtype patterns rather than bare primitives
- Wrapper types are not bypassed via direct field access outside their defining module
- Type aliases clarify intent without hiding complexity behind opaque names
- Error types give callers enough context to handle failures

### 5. Type Consistency Across Boundaries
- Parameter and return types of public functions are publicly accessible types;
  no private type leaks into a public signature
- The same semantic concept uses the same type consistently across module boundaries

## Pass Conditions

- No dangling references or ownership violations
- Generic bounds are sound (necessary and sufficient)
- All unsafe operations have documented preconditions
- Domain concepts use semantic wrapper/newtype types
- No private types leaked in public signatures

## Fail Conditions

- **Critical:** Dangling reference, use-after-free, or ownership violation
- **Critical:** Missing required generic bound causing unsoundness
- **Critical:** Unsafe operation with no justification comment
- **Critical:** Private type leaked into a public function signature
- **High:** Bare primitive used where a semantic wrapper type was specified
- **High:** Missing enum variant required by the domain model
- **High:** Struct field type, visibility, or name does not match the domain plan
- **Medium:** Unnecessary generic bound that over-constrains the API
- **Medium:** Incorrect or missing derive macro for a type's intended usage

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

See `4-review-type-validation` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for
language-specific syntax, bound rules, unsafe requirements, newtype patterns,
and checker logic.
