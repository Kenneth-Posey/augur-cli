---
name: rust-4-review-completeness-validation
description: >
  Rust-specific completeness validation for package manifests, file structure,
  implementation coverage, test harness presence, and checksum accuracy. Use
  when verifying that all planned artifacts are present, implemented, and
  traceable.
---

# Rust 4 Review Completeness Validation

**Authority boundary**: Structural completeness and traceability only. Do not
use this skill for behavioral correctness, type safety, naming conventions,
performance, or security review.

## Validation Role

This skill reviews Rust completeness by interpreting changed artifacts,
repo-local authorities, and any deterministic inventory evidence together, then
emits the shared `pass|fail` signal.

## Key Files

- `README.md` - overview and usage notes

## Scope

### What This Skill Validates

1. **Package Structure**
    - Package manifest (`Cargo.toml`) exists and is well-formed
    - Directory structure matches `.github/local/directories.md` and any
      feature-specific paths named in
      `plans/<feature-slug>/plan/implementation-plan.md`
    - No expected source or test path from those authorities is missing or
      misnamed

2. **Domain Coverage**
    - Each domain named in `plans/<feature-slug>/plan/domain-spec.md` has a
      corresponding implementation file
    - Domain files are non-empty (not stub-only)
    - No domain file is suspiciously small (< 1 KB flagged)

3. **Function Implementation Coverage**
    - Every function listed in
      `plans/<feature-slug>/plan/function-sig-plan.md` or
      `plans/<feature-slug>/plan/implementation-plan.md` has an implementation
    - No function body contains `todo!()` or `unimplemented!()`
    - Implementation files are non-trivial (functions file < 2 KB flagged)

4. **Test Harness**
   - A `tests/` directory (or inline test modules) exists
   - At least one test file is present and non-empty
   - Test files are non-trivial (< 1 KB flagged)
   - Behavior test files non-trivial (< 1 KB flagged)

5. **Checksums and Cross-References**
   - Checksums are recalculated and match the validation report
   - All cross-references in the manifest resolve to real files, types, and tests
   - No broken internal references

6. **Plan Traceability**
    - Every artifact can be traced back to
      `plans/<feature-slug>/plan/domain-spec.md`,
      `plans/<feature-slug>/plan/function-sig-plan.md`,
      `plans/<feature-slug>/plan/test-strategy-plan.md`, or
      `plans/<feature-slug>/plan/implementation-plan.md`
    - No scope creep: no code exists that has no corresponding item in those
      handoff files
    - No unimplemented requirements: no handoff item lacks a corresponding
      artifact, test, or manifest entry

7. **Uniqueness**
   - No duplicate type definitions
   - No duplicate function definitions across modules
   - Manifest totals are accurate (file counts, function counts)

### Coverage Boundaries

This skill assumes:
- The codebase compiles without errors
- The review handoff includes
  `plans/<feature-slug>/plan/domain-spec.md`,
  `plans/<feature-slug>/plan/function-sig-plan.md`,
  `plans/<feature-slug>/plan/test-strategy-plan.md`, and
  `plans/<feature-slug>/plan/implementation-plan.md`
- `Cargo.toml` and any deterministic inventory or checksum output used for the
  review are current for the tree being validated

## Validation Inputs

- Changed source files, test files, and `Cargo.toml`
- `plans/<feature-slug>/plan/domain-spec.md`
- `plans/<feature-slug>/plan/function-sig-plan.md`
- `plans/<feature-slug>/plan/test-strategy-plan.md`
- `plans/<feature-slug>/plan/implementation-plan.md`
- `.github/local/directories.md`
- Deterministic inventory, checksum, or manifest-validation output when
  provided as review evidence

## Review Output

- Missing-artifact findings linked to the exact governing handoff file
- Warnings for suspiciously thin files or incomplete tests
- `pass|fail` conclusion based on whether all required artifacts and
  references exist

## Validation Signal

Use the shared `pass|fail` vocabulary. Base the signal on review
judgment over the code and evidence set.

| Severity | Signal |
|----------|--------|
| Critical or High findings present | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |
