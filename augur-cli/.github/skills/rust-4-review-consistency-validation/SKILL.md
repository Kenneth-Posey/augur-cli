---
name: rust-4-review-consistency-validation
description: >
  Rust-specific consistency validation for naming conventions, documentation
  completeness, behavior-to-code alignment, and code style. Use when verifying
  that naming, docs, and code style are uniform and that each implementation
  matches its documented contract.
---

# Rust 4 Review Consistency Validation

**Authority boundary**: Naming, documentation, and style consistency only. Do not
use this skill for behavioral correctness, type safety, performance, or security
review.

## Review Role

Use this skill to assess Rust consistency by comparing changed code with
repo-local authorities and any deterministic documentation or AST evidence.
Return `pass` or `fail`.

## Key Files

- `README.md` - overview and usage notes

## Scope

### What This Skill Validates

1. **Naming Conventions**
   - Module and function names use `snake_case`
   - Type names (structs, enums, traits) use `PascalCase`
   - Constants and statics use `SCREAMING_SNAKE_CASE`
   - No deviations from Rust standard naming rules

2. **Documentation Completeness**
   - Every public function has a doc comment (`///`)
   - Every public type has a doc comment
   - Every public module has a doc comment
   - All public function parameters and return types are documented

3. **Behavior-to-Code Alignment**
    - Each function's implementation matches its documented behavior (error types,
      return values, side effects)
    - Each behavior's code path matches the Given/When/Then expectations in
      `plans/<feature-slug>/design/behaviors.md`
    - No undocumented side effects in public functions

4. **Scope Integrity**
    - No scope creep: no code present that is absent from
      `plans/<feature-slug>/plan/implementation-plan.md`,
      `plans/<feature-slug>/plan/function-sig-plan.md`, or
      `plans/<feature-slug>/design/behaviors.md`
    - No handoff gaps: no named behavior, signature, or public API item from
      those files is absent from the code
    - Error variants used correctly and match expected `Result` types
    - Unused error variants are flagged

5. **Doc Examples**
   - Doc examples compile without errors
   - Doc examples demonstrate correct usage of the function or type

6. **Code Style**
   - Indentation uses spaces, not tabs
   - Line length does not exceed 120 characters

### Coverage Boundaries

This skill assumes:
- The codebase compiles without errors
- The review handoff includes the relevant repo-local authorities:
  `plans/<feature-slug>/design/behaviors.md`,
  `plans/<feature-slug>/plan/function-sig-plan.md`, and
  `plans/<feature-slug>/plan/implementation-plan.md`
- Public API surface is already defined in those handoff files or the changed
  code under review (not designing new API)

## Validation Inputs

- Changed Rust source files, doc comments, and any public-facing examples in scope
- `plans/<feature-slug>/design/behaviors.md` for documented scenarios and outputs
- `plans/<feature-slug>/plan/function-sig-plan.md` for exported names and
  signature-level contracts
- `plans/<feature-slug>/plan/implementation-plan.md` for approved scope
- `.github/local/directories.md` for canonical naming and file-placement rules

## Review Output

- Findings tied to the exact file, symbol, and governing handoff artifact
- Warnings for consistency drift that does not block review
- Failures for undocumented public API, naming drift, or behavior/doc mismatches

## Validation Signal

Use the same `pass|fail` vocabulary as deterministic review tools.

| Severity | Signal |
|----------|--------|
| Critical or High findings present | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |
