---
name: rust-4-review-security-validation
description: >
  Rust-specific security pattern validation for unsafe block documentation,
  input validation, integer safety, secret handling, and injection prevention.
  Use when verifying that implementation follows Rust security best practices.
---

# Rust 4 Review Security Validation

**Authority boundary**: Security patterns and defensive coding only. Do not use
this skill for behavioral correctness, type safety, performance, naming
conventions, or style review.

## Role

Use this skill for security-focused review of Rust changes. Review the changed
code with repo-local authorities and any deterministic security artifacts, then
emit the shared `pass|fail` signal.

## Key Files

- `README.md` - overview and usage notes

## Scope

### What This Skill Validates

1. **Unsafe Block Safety**
   - Every `unsafe { ... }` block has a `// SAFETY:` comment documenting preconditions
   - Safety preconditions are specific and verifiable, not generic placeholders
   - No `unsafe` block without justification

2. **Input Validation**
   - Public functions accepting external input validate bounds, length, and encoding
     before use
   - String inputs validated as UTF-8 where encoding matters
   - Buffer operations check bounds before indexing

3. **Injection Prevention**
   - No string concatenation used to construct SQL queries (SQL injection risk)
   - No shell execution with unsanitized user input
   - File path operations validate against directory traversal attacks

4. **Integer Safety**
   - Integer operations use `checked_*`, `saturating_*`, or `wrapping_*` where overflow
     is possible
   - No silent integer overflow in arithmetic that affects security boundaries
   - No unbounded allocations that could cause denial-of-service

5. **Secret Handling**
   - No hardcoded credentials, API keys, or secrets in source
   - Error messages do not expose secrets, internal file paths, or database URLs
   - Secrets are not logged or printed to stdout/stderr
   - Secrets are cleared from memory after use

6. **Cryptography**
   - Correct algorithms used: SHA-256 or stronger (not MD5 or SHA-1)
   - Minimum 256-bit key length for symmetric encryption
   - No custom cryptographic implementations

7. **Panic Safety in Library Code**
   - No `unwrap()`, `expect()`, or `panic!()` in production library code
   - Test code and binary entrypoints are exempt
   - Use `?` or explicit error handling instead

### Coverage Boundaries

This skill assumes:
- The codebase compiles without errors
- The review scope is limited to static analysis (no runtime fuzzing)
- Threat model, trust boundaries, or sensitive data paths are documented in
  `plans/<feature-slug>/design/behaviors.md`,
  `plans/<feature-slug>/plan/domain-spec.md`, or
  `plans/<feature-slug>/plan/implementation-plan.md` where relevant

## Validation Inputs

- Changed Rust files, especially unsafe code, input parsing, and secret-handling paths
- `plans/<feature-slug>/design/behaviors.md` for externally triggered flows and
  failure modes
- `plans/<feature-slug>/plan/domain-spec.md` for invariants on trusted vs.
  untrusted data
- `plans/<feature-slug>/plan/implementation-plan.md` for declared unsafe, FFI,
  secret-bearing, or privileged execution surfaces
- `.github/local/directories.md` for distinguishing library, binary, and test paths
- Deterministic static-analysis output when provided as review input

## Review Output

- Security findings tied to the exact code location and governing handoff artifact
- Required follow-up for missing threat-model detail in repo-local handoff files
- A `pass|fail` conclusion that separates exploitable issues from
  documentation gaps

## Validation Signal

Use the same `pass|fail` vocabulary as the deterministic review-tool
layer.

| Severity | Signal |
|----------|--------|
| Critical or High findings present | `fail` |
| Medium or Low findings only | `pass` with warnings |
| Validation timed out | `fail` |
