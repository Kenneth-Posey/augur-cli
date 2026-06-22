---
name: 4-review-completeness-validation
description: >
  Universal completeness validation contract for Stage 4. Defines what artifact
  presence, implementation coverage, test harness existence, checksum integrity,
  and plan traceability must be verified, independent of language. Use at Stage 4
  when confirming all planned artifacts are present, implemented, and traceable
  before marking a phase done.
---

# Skill: 4-Review Completeness Validation

## Purpose

Validate that every artifact in the Stage 3 implementation plan is present,
non-trivial, traceable to a plan item, and free of stub placeholders.

## Key Files

- `README.md` - overview and usage notes

## What to Validate (Language-Agnostic Contract)

> **Coverage Matrix:** Do not reproduce the BEH→test mapping table. It is
> owned by the behavior-checker. State: "Full BEH→test mapping: see
> reports/behavior-report.md" and report only the pass/fail count summary.

### 1. Package / Project Structure
- The project manifest exists and is well-formed
- Directory structure matches the layout specified in the plan
- No required subdirectory is missing or misnamed

### 2. Domain Coverage
- Each domain or module in the plan has a corresponding implementation file
- Implementation files are non-empty and non-trivial (not stub-only)
- No domain file is suspiciously small relative to its planned scope

### 3. Function / Procedure Implementation Coverage
- Each function or procedure in the plan has a corresponding implementation
- No function body contains unfinished placeholder markers (`todo`, `unimplemented`,
  `not_yet_implemented`, or equivalent)
- Implementation files meet minimum size thresholds for non-trivial content

### 4. Test Harness Presence
- A test directory or inline test module exists
- At least one test file is present and non-empty
- Behavior test files are non-trivial

### 5. Checksum and Cross-Reference Integrity
- Checksums are recalculated and match the validation report
- All cross-references in the manifest resolve to real files, types, and tests
- No broken internal references

### 6. Plan Traceability
- Every artifact can be traced back to a plan requirement
- No scope creep: every code artifact maps to a plan item
- No unimplemented requirements: every plan item maps to an artifact

### 7. Uniqueness
- No duplicate type or function definitions across modules
- Manifest totals (file counts, function counts) are accurate

## Pass Conditions

- All planned artifacts are present and non-trivial
- No stub macros or unfinished placeholders in production code
- Test harness exists with at least one non-trivial test file
- Checksums match; all cross-references resolve
- Full bidirectional traceability between plan and code

## Fail Conditions

- **Critical:** Missing required domain or function implementation
- **Critical:** Unfinished placeholder found in production code
- **Critical:** Checksum mismatch or broken cross-reference
- **High:** Missing or empty test harness
- **High:** Plan item has no corresponding artifact (unimplemented requirement)
- **High:** Artifact exists with no plan item (scope creep)
- **Medium:** Suspiciously small implementation or test file

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

See `4-review-completeness-validation` in
[`.github/local/language-companions.md`](../../local/language-companions.md) for
language-specific manifest format, stub-macro detection rules, file size
thresholds, and checker logic.
